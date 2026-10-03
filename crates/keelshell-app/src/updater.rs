//! GitHub release discovery and reviewable self-update staging.
//!
//! Network work and archive writes run away from the UI thread. Automatic
//! binary replacement is deliberately unavailable until the native installer
//! and rollback path have platform acceptance; verified downloads remain
//! reviewable and never overwrite a running installation.

use std::{env, fs, path::PathBuf, sync::Arc, time::Duration};

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
}

#[derive(Debug, Clone)]
enum PanelState {
    Idle,
    Checking,
    UpToDate(ReleaseInfo),
    Available(ReleaseInfo),
    Downloading(ReleaseInfo),
    Ready(StagedUpdate),
    Failed,
}

pub enum UpdatePanelEvent {
    Close,
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
            PanelState::Checking | PanelState::Downloading(_)
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
                            "{} 已下载且 SHA-256 校验通过。此版本尚未启用自动替换安装。",
                            staged.release.tag
                        ),
                        format!(
                            "Downloaded and verified {}. The package is ready to view.",
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
        let (release, can_download, can_reveal) = match &self.state {
            PanelState::Available(release) | PanelState::UpToDate(release) => (
                Some(release),
                !matches!(self.state, PanelState::UpToDate(_)),
                false,
            ),
            PanelState::Downloading(release) => (Some(release), false, false),
            PanelState::Ready(staged) => (Some(&staged.release), false, true),
            _ => (None, false, false),
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
    // from replacing a predictable temporary path. Nothing is extracted.
    tokio::task::spawn_blocking(move || {
        let directory = env::temp_dir().join(format!(
            "keelshell-update-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&directory).map_err(|_| UpdateError::Storage)?;
        let archive = directory.join(&release.archive_name);
        fs::write(&archive, &bytes).map_err(|_| UpdateError::Storage)?;
        Ok(StagedUpdate {
            release,
            archive,
            digest: actual,
        })
    })
    .await
    .map_err(|_| UpdateError::Storage)?
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
        }
    }
}

#[cfg(test)]
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
}
