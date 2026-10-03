//! GitHub release discovery, verification, and opt-in self-update staging.
//!
//! Network work, archive extraction, and file hashing run away from the UI
//! thread. Installation is handed to a separate invocation of the same binary
//! after the user explicitly chooses “Install and restart”. The helper copies
//! only manifest-listed files, rolls back partial changes, and leaves unsigned
//! or unsupported installations reviewable instead of replacing them blindly.

use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::{
    assets::IconName,
    component::{
        Disableable,
        button::{Button, ButtonVariants},
    },
    prelude::FluentBuilder,
    *,
};
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::runtime::Runtime;

use crate::{
    design::{ACCENT, BORDER, CANVAS, MUTED, SURFACE, TEXT},
    i18n::{Message, t},
    runtime_bridge,
};

/// The public repository shown by the About panel and used for release APIs.
pub const PROJECT_URL: &str = "https://github.com/cyruss648/keelshell";
const RELEASES_API: &str = "https://api.github.com/repos/cyruss648/keelshell/releases/latest";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_RELEASE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const UPDATE_HELPER_ARG: &str = "--keelshell-apply-update";

#[derive(Debug, Clone, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone)]
struct ReleaseInfo {
    tag: String,
    name: String,
    body: String,
    url: String,
    published_at: Option<String>,
    archive_name: String,
    archive_url: String,
    checksum_url: String,
    archive_size: u64,
}

#[derive(Debug, Clone)]
struct StagedUpdate {
    release: ReleaseInfo,
    digest: String,
    archive: PathBuf,
    payload: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct PackageManifest {
    schema_version: u64,
    platform: String,
    version: String,
    #[serde(default)]
    target: Option<String>,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct UpdateFile {
    relative: PathBuf,
    digest: String,
}

#[derive(Debug, Clone)]
struct UpdatePlan {
    files: Vec<UpdateFile>,
}

#[derive(Debug, Clone)]
enum PanelState {
    Idle,
    Checking,
    UpToDate(ReleaseInfo),
    Available(ReleaseInfo),
    Downloading(ReleaseInfo),
    Ready(StagedUpdate),
    Installing,
    Failed,
}

pub enum UpdatePanelEvent {
    Close,
    Restart,
}

pub struct UpdatePanel {
    runtime: Arc<Runtime>,
    state: PanelState,
    current_target: Option<String>,
    status: Message,
    _job: Option<Task<()>>,
    focus: FocusHandle,
}

impl EventEmitter<UpdatePanelEvent> for UpdatePanel {}

impl UpdatePanel {
    pub fn new(runtime: Arc<Runtime>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            runtime,
            state: PanelState::Idle,
            current_target: target_triple(),
            status: Message::new(
                format!("当前版本 {CURRENT_VERSION}，可查看项目主页或检查最新版本。"),
                format!(
                    "Current version {CURRENT_VERSION}. Open the project or check for updates."
                ),
            ),
            _job: None,
            focus,
        }
    }

    fn check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.state,
            PanelState::Checking | PanelState::Downloading(_) | PanelState::Installing
        ) {
            return;
        }
        let Some(target) = self.current_target.clone() else {
            self.fail(
                "当前平台没有匹配的发布产物，请从项目主页手动下载。",
                "No release asset matches this platform. Download it from the project page.",
                cx,
            );
            return;
        };
        self.state = PanelState::Checking;
        self.status = Message::new("正在检查 GitHub Releases…", "Checking GitHub Releases…");
        let task = runtime_bridge::spawn(
            &self.runtime,
            cx.background_executor().clone(),
            async move { fetch_latest_release(&target).await },
        );
        self._job = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await.unwrap_or(Err(UpdateError::Network));
            let _ = this.update_in(cx, |panel, _window, cx| match result {
                Ok(release) if is_newer(&release.tag, CURRENT_VERSION) => {
                    panel.state = PanelState::Available(release);
                    panel.status = Message::new(
                        "发现新版本，可查看变更并下载校验。",
                        "A newer version is available. Review the changelog, then download it.",
                    );
                    cx.notify();
                }
                Ok(release) => {
                    panel.state = PanelState::UpToDate(release);
                    panel.status =
                        Message::new("当前已是最新版本。", "This installation is up to date.");
                    cx.notify();
                }
                Err(error) => {
                    panel.status = error.message();
                    panel.state = PanelState::Failed;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn download(&mut self, release: ReleaseInfo, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(
            self.state,
            PanelState::Available(_) | PanelState::UpToDate(_)
        ) {
            return;
        }
        self.state = PanelState::Downloading(release.clone());
        self.status = Message::new(
            "正在下载并校验发布产物…",
            "Downloading and verifying the release asset…",
        );
        let runtime = self.runtime.clone();
        let task = runtime_bridge::spawn(&runtime, cx.background_executor().clone(), async move {
            download_and_stage(release).await
        });
        self._job = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await.unwrap_or(Err(UpdateError::Network));
            let _ = this.update_in(cx, |panel, _window, cx| match result {
                Ok(staged) => {
                    panel.status = Message::new(
                        format!(
                            "{} 已下载且 SHA-256 校验通过，可自动安装并重启。",
                            staged.release.tag
                        ),
                        format!(
                            "Downloaded and verified {}. It is ready to install and restart.",
                            staged.release.tag
                        ),
                    );
                    panel.state = PanelState::Ready(staged);
                    cx.notify();
                }
                Err(error) => {
                    panel.status = error.message();
                    panel.state = PanelState::Failed;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn install(&mut self, staged: StagedUpdate, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.state, PanelState::Ready(_)) {
            return;
        }
        let Some(target) = self.current_target.clone() else {
            self.fail(
                "当前平台没有匹配的发布产物，无法自动安装。",
                "No release asset matches this platform, so automatic installation is unavailable.",
                cx,
            );
            return;
        };
        let current_exe = match env::current_exe() {
            Ok(path) => path,
            Err(_) => {
                self.fail(
                    "无法定位当前程序，自动安装已停止。",
                    "The current executable could not be located; automatic installation stopped.",
                    cx,
                );
                return;
            }
        };
        let Some(install_root) = installation_root(&current_exe, &target) else {
            self.fail(
                "当前程序不是可替换的已安装版本，请从发布页手动安装。",
                "This executable is not a replaceable installed package. Install manually from the release page.",
                cx,
            );
            return;
        };
        self.state = PanelState::Installing;
        self.status = Message::new(
            "正在启动安全安装助手；程序将关闭并在成功后重启。",
            "Starting the safe installer helper. The app will close and restart after a successful update.",
        );
        let payload = staged.payload;
        let current_exe_for_helper = current_exe.clone();
        let target_for_helper = target.clone();
        match spawn_update_helper(
            payload,
            install_root,
            current_exe_for_helper,
            target_for_helper,
        ) {
            Ok(()) => {
                // The helper owns the extracted payload after this point. The
                // old process exits before any destination file is changed.
                cx.notify();
                let _ = window;
                cx.emit(UpdatePanelEvent::Restart);
            }
            Err(error) => {
                self.state = PanelState::Failed;
                self.status = error.message();
                cx.notify();
            }
        }
    }

    fn fail(&mut self, zh: impl Into<String>, en: impl Into<String>, cx: &mut Context<Self>) {
        self.status = Message::new(zh, en);
        self.state = PanelState::Failed;
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.state,
            PanelState::Checking | PanelState::Downloading(_)
        ) {
            self.status = Message::new(
                "后台请求仍在进行；关闭面板不会安装未校验的文件。",
                "A background request is still running; closing this panel will not install an unverified file.",
            );
        }
        cx.emit(UpdatePanelEvent::Close);
    }
}

impl Render for UpdatePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (release, can_download, can_reveal, can_install) = match &self.state {
            PanelState::Available(release) | PanelState::UpToDate(release) => (
                Some(release),
                !matches!(self.state, PanelState::UpToDate(_)),
                false,
                false,
            ),
            PanelState::Downloading(release) => (Some(release), false, false, false),
            PanelState::Ready(staged) => (Some(&staged.release), false, true, true),
            _ => (None, false, false, false),
        };
        let body = release.map(|release| {
            let published = release
                .published_at
                .as_deref()
                .unwrap_or_else(|| t(cx, "未知时间", "Unknown date"));
            div()
                .id("update-release-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .bg(rgb(CANVAS))
                .border_1()
                .border_color(rgb(BORDER))
                .rounded(px(6.))
                .text_xs()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(TEXT))
                        .child(release.name.clone()),
                )
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child(format!("{} · {}", release.tag, published)),
                )
                .child(
                    div()
                        .mt_2()
                        .text_color(rgb(TEXT))
                        .child(release.body.clone()),
                )
        });
        let target = self
            .current_target
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_else(|| t(cx, "不支持的平台", "Unsupported platform").to_owned());
        div()
            .id("update-panel")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(SURFACE))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .flex_shrink_0()
                    .p_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .child(t(cx, "关于与更新", "About and updates")),
                    )
                    .child(
                        Button::new("close-update-panel")
                            .ghost()
                            .compact()
                            .label("×")
                            .on_click(cx.listener(|panel, _, _, cx| panel.close(cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().child(format!("KeelShell {CURRENT_VERSION}")))
                            .child(div().text_xs().text_color(rgb(MUTED)).child(target)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("open-project")
                                    .icon(IconName::ExternalLink)
                                    .ghost()
                                    .compact()
                                    .label(t(cx, "GitHub 项目主页", "GitHub project"))
                                    .on_click(|_, _, cx| cx.open_url(PROJECT_URL)),
                            )
                            .child(
                                Button::new("check-updates")
                                    .icon(IconName::RefreshCw)
                                    .primary()
                                    .compact()
                                    .label(t(cx, "检查更新", "Check for updates"))
                                    .disabled(matches!(
                                        self.state,
                                        PanelState::Checking | PanelState::Downloading(_)
                                    ))
                                    .on_click(
                                        cx.listener(|panel, _, window, cx| panel.check(window, cx)),
                                    ),
                            ),
                    )
                    .child(body.unwrap_or_else(|| {
                        div()
                            .flex_1()
                            .min_h_0()
                            .bg(rgb(CANVAS))
                            .rounded(px(6.))
                            .p_3()
                            .text_color(rgb(MUTED))
                            .id("bundled-changelog")
                            .overflow_y_scroll()
                            .child(include_str!("../../../CHANGELOG.md"))
                    }))
                    .when_some(
                        match &self.state {
                            PanelState::Ready(staged) => Some(staged.digest.clone()),
                            _ => None,
                        },
                        |body, digest| {
                            body.child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(format!("SHA-256: {digest}")),
                            )
                        },
                    )
                    .when_some(release.map(|release| release.url.clone()), |body, url| {
                        body.child(
                            Button::new("open-release")
                                .ghost()
                                .compact()
                                .label(t(cx, "在 GitHub 查看此版本", "View this release on GitHub"))
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(ACCENT))
                            .child(self.status.render(cx)),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p_3()
                    .border_t_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .justify_end()
                    .gap_2()
                    .when(can_download, |row| {
                        row.child(
                            Button::new("download-update")
                                .icon(IconName::Download)
                                .primary()
                                .label(t(cx, "下载并校验", "Download and verify"))
                                .on_click(cx.listener(|panel, _, window, cx| {
                                    if let PanelState::Available(release) = &panel.state {
                                        panel.download(release.clone(), window, cx)
                                    }
                                })),
                        )
                    })
                    .when(can_reveal, |row| {
                        row.child(
                            Button::new("reveal-update")
                                .icon(IconName::Check)
                                .primary()
                                .label(t(cx, "查看已校验的安装包", "Show verified package"))
                                .on_click(cx.listener(|panel, _, _, cx| {
                                    if let PanelState::Ready(staged) = &panel.state {
                                        cx.reveal_path(&staged.archive)
                                    }
                                })),
                        )
                    })
                    .when(can_install, |row| {
                        row.child(
                            Button::new("install-update")
                                .icon(IconName::Check)
                                .primary()
                                .label(t(cx, "自动安装并重启", "Install and restart"))
                                .on_click(cx.listener(|panel, _, window, cx| {
                                    if let PanelState::Ready(staged) = &panel.state {
                                        panel.install(staged.clone(), window, cx)
                                    }
                                })),
                        )
                    })
                    .child(
                        Button::new("cancel-update-panel")
                            .ghost()
                            .label(t(cx, "关闭", "Close"))
                            .on_click(cx.listener(|panel, _, _, cx| panel.close(cx))),
                    ),
            )
    }
}

async fn fetch_latest_release(target: &str) -> Result<ReleaseInfo, UpdateError> {
    let client = http_client(false)?;
    let response = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|_| UpdateError::Network)?;
    if response.status() != StatusCode::OK {
        return Err(UpdateError::Http(response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RELEASE_BYTES)
    {
        return Err(UpdateError::TooLarge);
    }
    let bytes = response.bytes().await.map_err(|_| UpdateError::Network)?;
    if bytes.len() as u64 > MAX_RELEASE_BYTES {
        return Err(UpdateError::TooLarge);
    }
    let release: GithubRelease =
        serde_json::from_slice(&bytes).map_err(|_| UpdateError::Invalid)?;
    release_info(release, target)
}

fn release_info(release: GithubRelease, target: &str) -> Result<ReleaseInfo, UpdateError> {
    if !release.tag_name.starts_with('v') || !valid_project_url(&release.html_url) {
        return Err(UpdateError::Invalid);
    }
    let archive_name = package_name(&release.tag_name, target).ok_or(UpdateError::Invalid)?;
    let archive = unique_asset(&release.assets, &archive_name)?;
    let checksum_name = format!("{archive_name}.sha256");
    let checksum = unique_asset(&release.assets, &checksum_name)?;
    if archive.size == 0
        || archive.size > MAX_ARCHIVE_BYTES
        || !valid_download_url(&archive.browser_download_url)
        || checksum.size == 0
        || checksum.size > 1024
        || !valid_download_url(&checksum.browser_download_url)
    {
        return Err(UpdateError::Invalid);
    }
    Ok(ReleaseInfo {
        tag: release.tag_name,
        name: release.name.unwrap_or_default(),
        body: release.body.unwrap_or_default(),
        url: release.html_url,
        published_at: release.published_at,
        archive_name,
        archive_url: archive.browser_download_url.clone(),
        checksum_url: checksum.browser_download_url.clone(),
        archive_size: archive.size,
    })
}

async fn download_and_stage(release: ReleaseInfo) -> Result<StagedUpdate, UpdateError> {
    let client = http_client(true)?;
    let checksum_response = client
        .get(&release.checksum_url)
        .send()
        .await
        .map_err(|_| UpdateError::Network)?;
    let checksum_bytes = read_bounded(checksum_response, 1024).await?;
    let checksum = std::str::from_utf8(&checksum_bytes).map_err(|_| UpdateError::Invalid)?;
    let expected = parse_checksum(checksum, &release.archive_name).ok_or(UpdateError::Invalid)?;
    let response = client
        .get(&release.archive_url)
        .send()
        .await
        .map_err(|_| UpdateError::Network)?;
    let bytes = read_bounded(response, release.archive_size).await?;
    if bytes.len() as u64 != release.archive_size {
        return Err(UpdateError::Invalid);
    }
    let actual = hex_digest(&Sha256::digest(&bytes));
    if expected != actual {
        return Err(UpdateError::Checksum);
    }
    // A private unique directory prevents other users or concurrent downloads
    // from replacing a predictable temporary path. Extracting before the
    // helper starts means the helper only copies files after the old process
    // has exited; it never needs to execute archive tooling.
    tokio::task::spawn_blocking(move || {
        let directory = env::temp_dir().join(format!(
            "keelshell-update-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&directory).map_err(|_| UpdateError::Storage)?;
        let archive = directory.join(&release.archive_name);
        fs::write(&archive, &bytes).map_err(|_| UpdateError::Storage)?;
        let payload = directory.join("payload");
        fs::create_dir(&payload).map_err(|_| UpdateError::Storage)?;
        extract_archive(&archive, &payload)?;
        let plan = validate_payload(&payload, &release)?;
        if plan.files.is_empty() {
            return Err(UpdateError::Invalid);
        }
        Ok(StagedUpdate {
            release,
            archive,
            payload,
            digest: actual,
        })
    })
    .await
    .map_err(|_| UpdateError::Storage)?
}

/// Runs the private helper mode used by an already spawned update process.
///
/// Returning `true` tells `main` that no GPUI window should be opened. The
/// helper revalidates the extracted package, copies only manifest-listed files,
/// rolls back on a partial failure, and relaunches the executable after the
/// operation. It is intentionally a command-line entry point rather than a
/// shell script so paths are passed as native arguments on every platform.
pub fn run_update_helper() -> bool {
    let helper_path = env::current_exe().ok();
    let mut args = env::args_os().skip(1);
    let Some(mode) = args.next() else {
        return false;
    };
    if mode != UPDATE_HELPER_ARG {
        return false;
    }
    let source = args.next().map(PathBuf::from);
    let install_root = args.next().map(PathBuf::from);
    let current_exe = args.next().map(PathBuf::from);
    let target = args.next().and_then(|value| value.into_string().ok());
    if args.next().is_some() {
        return true;
    }
    let (Some(source), Some(install_root), Some(current_exe), Some(target)) =
        (source, install_root, current_exe, target)
    else {
        return true;
    };
    let relaunch = current_exe.clone();
    let result = apply_update(&source, &install_root, &current_exe, &target);
    // A failed copy is rolled back by `install_plan`; relaunch the preserved
    // old executable so a transient file lock does not leave the user without
    // a running application.
    let _ = Command::new(result.unwrap_or(relaunch)).spawn();
    if helper_path
        .as_deref()
        .and_then(Path::file_name)
        .is_some_and(|name| {
            name.to_string_lossy()
                .starts_with("keelshell-update-helper-")
        })
    {
        let _ = helper_path.as_deref().map(fs::remove_file);
    }
    true
}

fn extract_archive(archive: &Path, destination: &Path) -> Result<(), UpdateError> {
    if archive
        .extension()
        .is_some_and(|extension| extension == "zip")
    {
        extract_zip(archive, destination)
    } else if archive
        .extension()
        .is_some_and(|extension| extension == "gz")
    {
        extract_tar_gz(archive, destination)
    } else {
        Err(UpdateError::Invalid)
    }
}

fn extract_zip(archive: &Path, destination: &Path) -> Result<(), UpdateError> {
    let file = File::open(archive).map_err(|_| UpdateError::Storage)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| UpdateError::Invalid)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(UpdateError::TooLarge);
    }
    let mut names = HashSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| UpdateError::Invalid)?;
        let raw_name = entry.name();
        // ZIP writers commonly add a trailing slash to directory members;
        // normalize only that directory marker while still rejecting empty
        // path components inside a member name.
        let name = if entry.is_dir() {
            raw_name.trim_end_matches('/')
        } else {
            raw_name
        };
        let Some(relative) = safe_relative(name) else {
            return Err(UpdateError::Invalid);
        };
        if !names.insert(relative.clone()) {
            return Err(UpdateError::Invalid);
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(UpdateError::Invalid);
        }
        let target = destination.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|_| UpdateError::Storage)?;
            continue;
        }
        let declared = entry.size();
        if declared > MAX_FILE_BYTES || total.saturating_add(declared) > MAX_UNPACKED_BYTES {
            return Err(UpdateError::TooLarge);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|_| UpdateError::Invalid)?;
        if bytes.len() as u64 != declared {
            return Err(UpdateError::Invalid);
        }
        total = total.saturating_add(declared);
        let mut output = File::create(&target).map_err(|_| UpdateError::Storage)?;
        output.write_all(&bytes).map_err(|_| UpdateError::Storage)?;
        output.sync_all().map_err(|_| UpdateError::Storage)?;
        set_extracted_mode(&target, entry.unix_mode());
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, destination: &Path) -> Result<(), UpdateError> {
    let file = File::open(archive).map_err(|_| UpdateError::Storage)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut names = HashSet::new();
    let mut total = 0_u64;
    let entries = archive.entries().map_err(|_| UpdateError::Invalid)?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ARCHIVE_ENTRIES {
            return Err(UpdateError::TooLarge);
        }
        let mut entry = entry.map_err(|_| UpdateError::Invalid)?;
        let raw = entry
            .path()
            .map_err(|_| UpdateError::Invalid)?
            .to_string_lossy()
            .into_owned();
        let Some(relative) = safe_relative(&raw) else {
            return Err(UpdateError::Invalid);
        };
        if !names.insert(relative.clone()) {
            return Err(UpdateError::Invalid);
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() || (!kind.is_file() && !kind.is_dir()) {
            return Err(UpdateError::Invalid);
        }
        let target = destination.join(&relative);
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|_| UpdateError::Storage)?;
            continue;
        }
        let declared = entry.header().size().map_err(|_| UpdateError::Invalid)?;
        if declared > MAX_FILE_BYTES || total.saturating_add(declared) > MAX_UNPACKED_BYTES {
            return Err(UpdateError::TooLarge);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
        }
        let mut output = File::create(&target).map_err(|_| UpdateError::Storage)?;
        let copied = io::copy(&mut entry, &mut output).map_err(|_| UpdateError::Invalid)?;
        if copied != declared {
            return Err(UpdateError::Invalid);
        }
        output.sync_all().map_err(|_| UpdateError::Storage)?;
        total = total.saturating_add(copied);
        set_extracted_mode(
            &target,
            Some(entry.header().mode().map_err(|_| UpdateError::Invalid)?),
        );
    }
    Ok(())
}

fn set_extracted_mode(path: &Path, mode: Option<u32>) {
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

fn validate_payload(payload: &Path, release: &ReleaseInfo) -> Result<UpdatePlan, UpdateError> {
    let manifest_path = payload.join("package-manifest.json");
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|_| UpdateError::Invalid)?;
    if !metadata.file_type().is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err(UpdateError::Invalid);
    }
    let manifest: PackageManifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|_| UpdateError::Storage)?)
            .map_err(|_| UpdateError::Invalid)?;
    let version = release.tag.strip_prefix('v').ok_or(UpdateError::Invalid)?;
    if manifest.schema_version != 1
        || manifest.platform != platform_name()
        || manifest.version != version
        || manifest.target.as_deref() != Some(release_target(&release.archive_name)?)
        || !is_newer(&release.tag, CURRENT_VERSION)
        || manifest.files.len() > MAX_ARCHIVE_ENTRIES
    {
        return Err(UpdateError::Invalid);
    }
    let mut total = 0_u64;
    let mut files = Vec::with_capacity(manifest.files.len());
    for (name, digest) in manifest.files {
        let Some(relative) = safe_relative(&name) else {
            return Err(UpdateError::Invalid);
        };
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(UpdateError::Invalid);
        }
        let path = payload.join(&relative);
        let metadata = fs::symlink_metadata(&path).map_err(|_| UpdateError::Invalid)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(UpdateError::Invalid);
        }
        total = total.saturating_add(metadata.len());
        if total > MAX_UNPACKED_BYTES || hex_file_digest(&path)? != digest.to_ascii_lowercase() {
            return Err(UpdateError::Invalid);
        }
        files.push(UpdateFile {
            relative,
            digest: digest.to_ascii_lowercase(),
        });
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(UpdatePlan { files })
}

fn release_target(archive_name: &str) -> Result<&str, UpdateError> {
    let prefix = archive_name
        .strip_prefix("KeelShell-")
        .ok_or(UpdateError::Invalid)?;
    target_triples()
        .into_iter()
        .find(|target| {
            prefix.ends_with(&format!("-{target}.zip"))
                || prefix.ends_with(&format!("-{target}.tar.gz"))
        })
        .ok_or(UpdateError::Invalid)
}

fn platform_name() -> &'static str {
    match env::consts::OS {
        "macos" => "macos",
        "linux" => "linux",
        "windows" => "windows",
        _ => "unknown",
    }
}

fn hex_file_digest(path: &Path) -> Result<String, UpdateError> {
    let mut file = File::open(path).map_err(|_| UpdateError::Storage)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| UpdateError::Storage)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex_digest(&digest.finalize()))
}

fn safe_relative(value: &str) -> Option<PathBuf> {
    if value.is_empty() || value.contains('\\') || value.contains('\0') || value.starts_with('/') {
        return None;
    }
    let mut path = PathBuf::new();
    for part in value.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return None;
        }
        path.push(part);
    }
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Some(path)
    } else {
        None
    }
}

fn installation_root(current_exe: &Path, target: &str) -> Option<PathBuf> {
    if !current_exe.is_absolute() {
        return None;
    }
    match target {
        target if target.contains("apple-darwin") => current_exe
            .ancestors()
            .find(|path| path.file_name().is_some_and(|name| name == "KeelShell.app"))
            .and_then(Path::parent)
            .map(Path::to_path_buf),
        target if target.contains("unknown-linux") => {
            let binary = Path::new("usr").join("bin").join("keelshell-app");
            let mut components = current_exe.components().collect::<Vec<_>>();
            let suffix = binary.components().collect::<Vec<_>>();
            if components.len() < suffix.len()
                || components.split_off(components.len() - suffix.len()) != suffix
            {
                return None;
            }
            let mut root = PathBuf::new();
            for component in components {
                root.push(component.as_os_str());
            }
            Some(root)
        }
        target if target.contains("windows-msvc") => current_exe.parent().map(Path::to_path_buf),
        _ => None,
    }
}

fn spawn_update_helper(
    payload: PathBuf,
    install_root: PathBuf,
    current_exe: PathBuf,
    target: String,
) -> Result<(), UpdateError> {
    if !payload.is_absolute() || !install_root.is_absolute() || !current_exe.is_absolute() {
        return Err(UpdateError::Install);
    }
    // Keep the helper executable outside the installation tree. Windows keeps
    // the running image locked, so launching the same path would prevent the
    // helper from replacing the old executable. A private copy also avoids
    // changing the image that is still serving the UI until the UI exits.
    let helper_name = format!(
        "keelshell-update-helper-{}-{}{}",
        std::process::id(),
        uuid::Uuid::new_v4(),
        if cfg!(windows) { ".exe" } else { "" }
    );
    let helper = env::temp_dir().join(helper_name);
    fs::copy(&current_exe, &helper).map_err(|_| UpdateError::Install)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = match fs::metadata(&current_exe) {
            Ok(metadata) => metadata.permissions().mode(),
            Err(_) => {
                let _ = fs::remove_file(&helper);
                return Err(UpdateError::Install);
            }
        };
        if fs::set_permissions(&helper, fs::Permissions::from_mode(mode & 0o777)).is_err() {
            let _ = fs::remove_file(&helper);
            return Err(UpdateError::Install);
        }
    }
    let result = Command::new(&helper)
        .arg(UPDATE_HELPER_ARG)
        .arg(payload)
        .arg(install_root)
        .arg(current_exe)
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|_| UpdateError::Install);
    if result.is_err() {
        let _ = fs::remove_file(helper);
    }
    result
}

fn apply_update(
    payload: &Path,
    install_root: &Path,
    current_exe: &Path,
    target: &str,
) -> Result<PathBuf, UpdateError> {
    if !payload.is_absolute() || !install_root.is_absolute() || !current_exe.is_absolute() {
        return Err(UpdateError::Install);
    }
    let payload_metadata = fs::symlink_metadata(payload).map_err(|_| UpdateError::Install)?;
    let install_metadata = fs::symlink_metadata(install_root).map_err(|_| UpdateError::Install)?;
    let executable_metadata =
        fs::symlink_metadata(current_exe).map_err(|_| UpdateError::Install)?;
    if !payload_metadata.file_type().is_dir()
        || !install_metadata.file_type().is_dir()
        || !executable_metadata.file_type().is_file()
        || !current_exe.starts_with(install_root)
    {
        return Err(UpdateError::Install);
    }
    // Helper mode does not receive the release API object. Read and validate
    // the manifest directly while retaining the target and version checks.
    let plan = validate_payload_for_helper(payload, target)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match install_plan(payload, install_root, &plan) {
            Ok(()) => return Ok(current_exe.to_path_buf()),
            Err(UpdateError::Busy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => return Err(error),
        }
    }
}

fn validate_payload_for_helper(payload: &Path, target: &str) -> Result<UpdatePlan, UpdateError> {
    let path = payload.join("package-manifest.json");
    let metadata = fs::symlink_metadata(&path).map_err(|_| UpdateError::Invalid)?;
    if !metadata.file_type().is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err(UpdateError::Invalid);
    }
    let manifest: PackageManifest =
        serde_json::from_slice(&fs::read(&path).map_err(|_| UpdateError::Storage)?)
            .map_err(|_| UpdateError::Invalid)?;
    let expected_target = target_triples()
        .into_iter()
        .find(|candidate| *candidate == target)
        .ok_or(UpdateError::Invalid)?;
    if manifest.schema_version != 1
        || manifest.platform != platform_name()
        || manifest.target.as_deref() != Some(expected_target)
        || !is_newer(&format!("v{}", manifest.version), CURRENT_VERSION)
        || manifest.files.len() > MAX_ARCHIVE_ENTRIES
    {
        return Err(UpdateError::Invalid);
    }
    let mut total = 0_u64;
    let mut files = Vec::with_capacity(manifest.files.len());
    for (name, digest) in manifest.files {
        let relative = safe_relative(&name).ok_or(UpdateError::Invalid)?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(UpdateError::Invalid);
        }
        let source = payload.join(&relative);
        ensure_no_symlink_ancestors(payload, &source)?;
        let metadata = fs::symlink_metadata(&source).map_err(|_| UpdateError::Invalid)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(UpdateError::Invalid);
        }
        total = total.saturating_add(metadata.len());
        if total > MAX_UNPACKED_BYTES || hex_file_digest(&source)? != digest.to_ascii_lowercase() {
            return Err(UpdateError::Invalid);
        }
        files.push(UpdateFile {
            relative,
            digest: digest.to_ascii_lowercase(),
        });
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(UpdatePlan { files })
}

fn install_plan(payload: &Path, install_root: &Path, plan: &UpdatePlan) -> Result<(), UpdateError> {
    let backup_root = payload.join(".backup");
    if backup_root.exists() {
        fs::remove_dir_all(&backup_root).map_err(|_| UpdateError::Storage)?;
    }
    fs::create_dir(&backup_root).map_err(|_| UpdateError::Storage)?;
    let mut installed = Vec::new();
    let mut backups = Vec::new();
    for file in &plan.files {
        let result = (|| {
            let source = payload.join(&file.relative);
            let destination = install_root.join(&file.relative);
            if !destination.starts_with(install_root) || !source.starts_with(payload) {
                return Err(UpdateError::Install);
            }
            ensure_no_symlink_ancestors(install_root, &destination)?;
            let source_digest = hex_file_digest(&source)?;
            if source_digest != file.digest {
                return Err(UpdateError::Checksum);
            }
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
            }
            if fs::symlink_metadata(&destination).is_ok() {
                let metadata = fs::symlink_metadata(&destination).map_err(|_| UpdateError::Busy)?;
                if !metadata.file_type().is_file() {
                    return Err(UpdateError::Install);
                }
                let backup = backup_root.join(&file.relative);
                if let Some(parent) = backup.parent() {
                    fs::create_dir_all(parent).map_err(|_| UpdateError::Storage)?;
                }
                fs::rename(&destination, &backup).map_err(|_| UpdateError::Busy)?;
                backups.push((destination.clone(), backup));
            }
            copy_new_file(&source, &destination)?;
            Ok(destination)
        })();
        match result {
            Ok(destination) => installed.push(destination),
            Err(error) => {
                rollback_install(&installed, &backups);
                return Err(error);
            }
        }
    }
    Ok(())
}

fn ensure_no_symlink_ancestors(root: &Path, destination: &Path) -> Result<(), UpdateError> {
    let relative = destination
        .strip_prefix(root)
        .map_err(|_| UpdateError::Install)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        if current.exists()
            && fs::symlink_metadata(&current)
                .map_err(|_| UpdateError::Busy)?
                .file_type()
                .is_symlink()
        {
            return Err(UpdateError::Install);
        }
    }
    Ok(())
}

fn copy_new_file(source: &Path, destination: &Path) -> Result<(), UpdateError> {
    let result = (|| {
        let mut input = File::open(source).map_err(|_| UpdateError::Storage)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    UpdateError::Busy
                } else {
                    UpdateError::Storage
                }
            })?;
        io::copy(&mut input, &mut output).map_err(|_| UpdateError::Storage)?;
        output.sync_all().map_err(|_| UpdateError::Storage)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(mode) = input
                .metadata()
                .map(|metadata| metadata.permissions().mode())
            {
                let _ = fs::set_permissions(destination, fs::Permissions::from_mode(mode & 0o777));
            }
        }
        Ok(())
    })();
    if result.is_err() {
        // A failed copy may have created a partial destination. Remove it so
        // rollback can restore the previous file from the backup.
        let _ = fs::remove_file(destination);
    }
    result
}

fn rollback_install(installed: &[PathBuf], backups: &[(PathBuf, PathBuf)]) {
    for path in installed.iter().rev() {
        let _ = fs::remove_file(path);
    }
    for (destination, backup) in backups.iter().rev() {
        let _ = fs::rename(backup, destination);
    }
}

async fn read_bounded(
    mut response: reqwest::Response,
    maximum: u64,
) -> Result<Vec<u8>, UpdateError> {
    if response.status() != StatusCode::OK {
        return Err(UpdateError::Http(response.status().as_u16()));
    }
    if response.content_length().is_some_and(|size| size > maximum) {
        return Err(UpdateError::TooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| UpdateError::Network)? {
        if chunk.len() as u64 > maximum.saturating_sub(bytes.len() as u64) {
            return Err(UpdateError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unique_asset<'a>(assets: &'a [GithubAsset], name: &str) -> Result<&'a GithubAsset, UpdateError> {
    let mut matching = assets.iter().filter(|asset| asset.name == name);
    let asset = matching.next().ok_or(UpdateError::AssetMissing)?;
    if matching.next().is_some() {
        return Err(UpdateError::Invalid);
    }
    Ok(asset)
}

fn http_client(download: bool) -> Result<reqwest::Client, UpdateError> {
    let redirects = if download {
        reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3 || !allowed_download_redirect(attempt.url()) {
                attempt.error("release redirect is not allowed")
            } else {
                attempt.follow()
            }
        })
    } else {
        reqwest::redirect::Policy::none()
    };
    reqwest::Client::builder()
        .timeout(Duration::from_secs(if download { 300 } else { 20 }))
        .connect_timeout(Duration::from_secs(8))
        .redirect(redirects)
        .retry(reqwest::retry::never())
        .user_agent(format!("KeelShell/{CURRENT_VERSION}"))
        .build()
        .map_err(|_| UpdateError::Network)
}

fn allowed_download_redirect(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|port| port == 443)
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn valid_project_url(url: &str) -> bool {
    url.starts_with("https://github.com/cyruss648/keelshell/")
}

fn valid_download_url(url: &str) -> bool {
    url.starts_with("https://github.com/cyruss648/keelshell/releases/download/")
}

fn package_name(tag: &str, target: &str) -> Option<String> {
    let version = tag.strip_prefix('v')?;
    if parse_version(version).is_none() || !target_triples().contains(&target) {
        return None;
    }
    let extension = if target.contains("unknown-linux") {
        "tar.gz"
    } else {
        "zip"
    };
    Some(format!("KeelShell-{version}-{target}.{extension}"))
}

fn target_triple() -> Option<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    match (os, arch) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin".into()),
        ("macos", "x86_64") => Some("x86_64-apple-darwin".into()),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu".into()),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu".into()),
        ("windows", "aarch64") => Some("aarch64-pc-windows-msvc".into()),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc".into()),
        _ => None,
    }
}

fn target_triples() -> [&'static str; 6] {
    [
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
        "aarch64-pc-windows-msvc",
        "x86_64-pc-windows-msvc",
    ]
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let values: Vec<_> = value.split('.').collect();
    if values.len() != 3
        || values.iter().any(|part| {
            part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return None;
    }
    Some((
        values[0].parse().ok()?,
        values[1].parse().ok()?,
        values[2].parse().ok()?,
    ))
}

fn is_newer(remote: &str, current: &str) -> bool {
    remote
        .strip_prefix('v')
        .and_then(parse_version)
        .zip(parse_version(current))
        .is_some_and(|(remote, current)| remote > current)
}

fn parse_checksum(value: &str, file_name: &str) -> Option<String> {
    let mut lines = value.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next()?.trim();
    if lines.next().is_some() {
        return None;
    }
    let mut fields = line.split_whitespace();
    let checksum = fields.next()?.to_ascii_lowercase();
    let name = fields.next()?.trim_start_matches('*');
    if fields.next().is_some()
        || checksum.len() != 64
        || !checksum.chars().all(|c| c.is_ascii_hexdigit())
        || name != file_name
    {
        return None;
    }
    Some(checksum)
}

#[derive(Debug, Clone, Copy)]
enum UpdateError {
    Network,
    Http(u16),
    TooLarge,
    Invalid,
    AssetMissing,
    Checksum,
    Storage,
    Busy,
    Install,
}

impl UpdateError {
    fn message(self) -> Message {
        match self {
            Self::Network => Message::new(
                "网络请求失败或超时，请稍后重试。",
                "The network request failed or timed out. Try again later.",
            ),
            Self::Http(status) => Message::detail(
                "发布服务返回 HTTP 状态",
                "Release service returned HTTP status",
                status,
            ),
            Self::AssetMissing => Message::new(
                "此版本缺少当前平台安装包或校验文件，请查看 GitHub 发布页。",
                "The release has no matching platform asset or checksum. See the GitHub release page.",
            ),
            Self::TooLarge => Message::new(
                "响应超过大小上限，已停止。",
                "The response exceeded its size limit and was stopped.",
            ),
            Self::Invalid => Message::new(
                "发布元数据或校验文件无效，已停止。",
                "Release metadata or checksum data was invalid. The operation stopped.",
            ),
            Self::Checksum => Message::new(
                "SHA-256 校验不匹配，未保存安装包。",
                "SHA-256 mismatch. The package was not saved.",
            ),
            Self::Storage => Message::new(
                "无法保存已校验安装包，请检查本机存储空间。",
                "Unable to save the verified package. Check local storage.",
            ),
            Self::Busy => Message::new(
                "当前安装目录正在使用中，自动安装已回滚；请稍后重试。",
                "The installation directory is busy. The update was rolled back; try again later.",
            ),
            Self::Install => Message::new(
                "自动安装失败，原安装未被替换。请从发布页手动安装。",
                "Automatic installation failed and the existing installation was preserved. Install manually from the release page.",
            ),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn package_names_bind_version_and_target() {
        assert_eq!(
            package_name("v1.2.3", "x86_64-unknown-linux-gnu").as_deref(),
            Some("KeelShell-1.2.3-x86_64-unknown-linux-gnu.tar.gz")
        );
        assert!(package_name("v1.2", "x86_64-unknown-linux-gnu").is_none());
        assert!(package_name("v1.2.3", "unknown").is_none());
    }

    #[::core::prelude::v1::test]
    fn update_comparison_rejects_invalid_or_older_versions() {
        assert!(is_newer("v1.2.4", "1.2.3"));
        assert!(!is_newer("v1.2.3", "1.2.3"));
        assert!(!is_newer("v1.2.2", "1.2.3"));
        assert!(!is_newer("release", "1.2.3"));
    }

    #[::core::prelude::v1::test]
    fn checksum_parser_requires_exact_filename_and_line() {
        let digest = "a".repeat(64);
        assert_eq!(
            parse_checksum(&format!("{digest}  file.zip\n"), "file.zip"),
            Some(digest.clone())
        );
        assert_eq!(
            parse_checksum(&format!("{digest} *file.zip\n"), "file.zip"),
            Some(digest)
        );
        assert!(parse_checksum(&format!("{}  other.zip", "b".repeat(64)), "file.zip").is_none());
        assert!(
            parse_checksum(&format!("{}  file.zip\nextra", "c".repeat(64)), "file.zip").is_none()
        );
    }

    #[::core::prelude::v1::test]
    fn release_urls_are_pinned_to_github_repository() {
        assert!(valid_project_url(
            "https://github.com/cyruss648/keelshell/releases/tag/v1.0.0"
        ));
        assert!(valid_download_url(
            "https://github.com/cyruss648/keelshell/releases/download/v1.0.0/file.zip"
        ));
        assert!(!valid_download_url("https://example.test/file.zip"));
    }

    #[::core::prelude::v1::test]
    fn archive_target_and_relative_paths_are_strict() {
        assert_eq!(
            release_target("KeelShell-1.2.3-x86_64-unknown-linux-gnu.tar.gz").ok(),
            Some("x86_64-unknown-linux-gnu")
        );
        assert_eq!(
            release_target("KeelShell-1.2.3-aarch64-apple-darwin.zip").ok(),
            Some("aarch64-apple-darwin")
        );
        for value in [
            "../escape",
            "/absolute",
            "a/../b",
            "a\\b",
            "C:drive",
            "./name",
        ] {
            assert!(safe_relative(value).is_none(), "{value:?}");
        }
        assert_eq!(
            safe_relative("KeelShell.app/Contents/MacOS/keelshell-app"),
            Some(PathBuf::from("KeelShell.app/Contents/MacOS/keelshell-app"))
        );
    }

    #[::core::prelude::v1::test]
    fn zip_directory_markers_are_normalized_without_relaxing_paths() {
        let directory = tempfile::tempdir().expect("temporary archive directory");
        let archive_path = directory.path().join("package.zip");
        let destination = directory.path().join("payload");
        std::fs::create_dir(&destination).expect("payload");
        let file = File::create(&archive_path).expect("archive");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .add_directory("KeelShell.app/", options)
            .expect("directory");
        writer
            .start_file("KeelShell.app/Contents/file", options)
            .expect("file");
        writer.write_all(b"content").expect("file content");
        writer.finish().expect("archive finish");
        extract_archive(&archive_path, &destination).expect("extract");
        assert_eq!(
            std::fs::read(destination.join("KeelShell.app/Contents/file")).expect("extracted"),
            b"content"
        );
    }

    #[::core::prelude::v1::test]
    fn installation_roots_follow_packaged_layouts() {
        assert_eq!(
            installation_root(
                Path::new("/opt/keelshell/usr/bin/keelshell-app"),
                "x86_64-unknown-linux-gnu"
            ),
            Some(PathBuf::from("/opt/keelshell"))
        );
        assert_eq!(
            installation_root(
                Path::new("/Applications/KeelShell.app/Contents/MacOS/keelshell-app"),
                "aarch64-apple-darwin"
            ),
            Some(PathBuf::from("/Applications"))
        );
    }

    #[::core::prelude::v1::test]
    fn helper_manifest_checks_digest_and_target() {
        let directory = tempfile::tempdir().expect("temporary update directory");
        let payload = directory.path().join("payload");
        std::fs::create_dir_all(&payload).expect("payload");
        let binary = payload.join("keelshell-app.exe");
        std::fs::write(&binary, b"verified binary").expect("binary");
        let digest = hex_file_digest(&binary).expect("digest");
        let target = target_triple().expect("current release target");
        let manifest = serde_json::json!({
            "schema_version": 1,
            "platform": platform_name(),
            "version": "99.0.0",
            "target": target,
            "files": {"keelshell-app.exe": digest},
        });
        std::fs::write(
            payload.join("package-manifest.json"),
            serde_json::to_vec(&manifest).expect("manifest JSON"),
        )
        .expect("manifest");
        let plan = validate_payload_for_helper(&payload, &target).expect("valid manifest");
        assert_eq!(plan.files.len(), 1);
        std::fs::write(&binary, b"tampered").expect("tamper");
        assert!(matches!(
            validate_payload_for_helper(&payload, &target),
            Err(UpdateError::Invalid)
        ));
    }

    #[::core::prelude::v1::test]
    fn install_plan_rolls_back_replaced_files() {
        let directory = tempfile::tempdir().expect("temporary update directory");
        let payload = directory.path().join("payload");
        let install = directory.path().join("install");
        std::fs::create_dir_all(&payload).expect("payload");
        std::fs::create_dir_all(&install).expect("install");
        let source = payload.join("keelshell-app.exe");
        let destination = install.join("keelshell-app.exe");
        std::fs::write(&source, b"new").expect("source");
        std::fs::write(&destination, b"old").expect("destination");
        let plan = UpdatePlan {
            files: vec![UpdateFile {
                relative: PathBuf::from("keelshell-app.exe"),
                digest: hex_file_digest(&source).expect("source digest"),
            }],
        };
        install_plan(&payload, &install, &plan).expect("install");
        assert_eq!(std::fs::read(&destination).expect("installed file"), b"new");
        assert_eq!(
            std::fs::read(payload.join(".backup/keelshell-app.exe")).expect("backup"),
            b"old"
        );
    }
}
