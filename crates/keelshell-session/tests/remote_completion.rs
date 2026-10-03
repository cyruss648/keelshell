//! Real TCP/SSH/SFTP candidate queries; the fixture never executes shell text.
use keelshell_session::{
    CompletionError, CompletionKind, CompletionQuery, SshAuth, SshOptions, SshSession,
};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use russh_sftp::protocol::{
    Attrs, File, FileAttributes, Handle, Name, Status, StatusCode, Version,
};
use std::{
    collections::HashMap,
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    net::TcpListener,
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
const PROBE: &[u8] = br#"command printf 'KEELSHELL_COMPLETION_V1\000%s\000' "$PATH""#;
const HANDLE: &str = "\0\u{1}directory";
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Unsafe,
    Many,
    ScanLimit,
    ExactScan,
    ExactBytes,
    ByteLimit,
    StatLimit,
    EmptyPage,
    BadCanonical,
    Denied,
    StallRead,
    StallStat,
    StallInit,
    StallProbe,
    LateOpen,
    SlowSteps,
    BadUtf8,
    BadHandle,
    LongHandle,
    HugeNameCount,
    PoisonStatus,
}
#[derive(Default)]
struct Observed {
    execs: Mutex<Vec<Vec<u8>>>,
    paths: Mutex<Vec<String>>,
    active: AtomicUsize,
    opens: AtomicUsize,
    reads: AtomicUsize,
    stats: AtomicUsize,
    closes: AtomicUsize,
    channel_closes: AtomicUsize,
    unexpected: AtomicUsize,
}
#[derive(Clone)]
struct Config {
    mode: Mode,
    probe: Vec<u8>,
    exit: Option<u32>,
    stderr: Vec<u8>,
    observed: Arc<Observed>,
}
impl Config {
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            probe: b"KEELSHELL_COMPLETION_V1\0/bin::relative:/later:/denied:/bin\0".to_vec(),
            exit: Some(0),
            stderr: Vec::new(),
            observed: Arc::default(),
        }
    }
}
struct Filesystem {
    config: Config,
    offset: usize,
    directory: String,
}
impl Drop for Filesystem {
    fn drop(&mut self) {
        self.config.observed.active.fetch_sub(1, Ordering::AcqRel);
    }
}
fn file(name: impl Into<String>, mode: u32) -> File {
    let mut attrs = FileAttributes::empty();
    attrs.permissions = Some(mode);
    File {
        filename: name.into(),
        longname: "ignored peer format".into(),
        attrs,
    }
}
impl russh_sftp::server::Handler for Filesystem {
    type Error = StatusCode;
    fn unimplemented(&self) -> StatusCode {
        self.config
            .observed
            .unexpected
            .fetch_add(1, Ordering::AcqRel);
        StatusCode::OpUnsupported
    }
    async fn init(&mut self, _: u32, _: HashMap<String, String>) -> Result<Version, StatusCode> {
        if self.config.mode == Mode::StallInit {
            std::future::pending::<()>().await;
        }
        if self.config.mode == Mode::SlowSteps {
            tokio::time::sleep(Duration::from_millis(1800)).await;
        }
        Ok(Version::new())
    }
    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, StatusCode> {
        if self.config.mode == Mode::SlowSteps {
            tokio::time::sleep(Duration::from_millis(1800)).await;
        }
        self.config
            .observed
            .paths
            .lock()
            .map_err(|_| StatusCode::Failure)?
            .push(path.clone());
        if path == "/denied" || self.config.mode == Mode::Denied {
            return Err(StatusCode::PermissionDenied);
        }
        let path = if self.config.mode == Mode::BadCanonical {
            "relative\u{fffd}".into()
        } else if path == "." || path.starts_with("/literal/") {
            "/fixture".into()
        } else {
            path
        };
        Ok(Name {
            id,
            files: vec![File::dummy(path)],
        })
    }
    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, StatusCode> {
        self.directory = path;
        self.offset = 0;
        Ok(Handle {
            id,
            handle: HANDLE.into(),
        })
    }
    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, StatusCode> {
        assert_eq!(handle, HANDLE, "opaque handle must round-trip controls");
        self.config.observed.reads.fetch_add(1, Ordering::AcqRel);
        if self.config.mode == Mode::SlowSteps {
            tokio::time::sleep(Duration::from_millis(1800)).await;
        }
        if self.config.mode == Mode::StallRead {
            std::future::pending::<()>().await;
        }
        if self.config.mode == Mode::EmptyPage {
            return Ok(Name { id, files: vec![] });
        }
        let count = match self.config.mode {
            Mode::Many => 80,
            Mode::ScanLimit => 9000,
            Mode::ExactScan => 8192,
            Mode::ExactBytes => 4096,
            Mode::ByteLimit => 3000,
            Mode::StatLimit => 80,
            _ => 1,
        };
        if self.offset >= count {
            return Err(StatusCode::Eof);
        }
        let files = match self.config.mode {
            Mode::Many
            | Mode::ScanLimit
            | Mode::ExactScan
            | Mode::ExactBytes
            | Mode::ByteLimit
            | Mode::StatLimit => {
                let end = (self.offset + 100).min(count);
                let files = (self.offset..end)
                    .rev()
                    .map(|index| {
                        let name = if self.config.mode == Mode::ByteLimit {
                            format!("n{index:05}{}", "x".repeat(1010))
                        } else {
                            format!("n{index:05}")
                        };
                        let mut entry = file(
                            name,
                            if self.config.mode == Mode::StatLimit {
                                0o120777
                            } else {
                                0o100755
                            },
                        );
                        if self.config.mode == Mode::ExactBytes {
                            entry.longname = "x".repeat(506);
                        }
                        entry
                    })
                    .collect();
                self.offset = end;
                files
            }
            Mode::Unsafe => {
                self.offset = 1;
                vec![
                    file("safe 中文", 0o100644),
                    file("bad/name", 0o100644),
                    file("bad\nname", 0o100644),
                    file("bad\u{202e}", 0o100644),
                    file("bad\u{fffd}", 0o100644),
                    file(".", 0o040755),
                    file("..", 0o040755),
                    file("a'\"$()`;*?[]\\", 0o100644),
                    file("z".repeat(1025), 0o100644),
                ]
            }
            _ => {
                self.offset = 1;
                if self.directory == "/later" {
                    vec![file("alpha", 0o100755), file("later", 0o100755)]
                } else {
                    vec![
                        file("zeta", 0o100755),
                        file("directory", 0o040755),
                        file("alpha", 0o100755),
                        file("plain", 0o100644),
                        file("untyped-link", 0o755),
                        file("linkdir", 0o120777),
                        file("linkcmd", 0o120777),
                        file("broken", 0o120777),
                        file("socket", 0o140777),
                    ]
                }
            }
        };
        Ok(Name { id, files })
    }
    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        self.config.observed.stats.fetch_add(1, Ordering::AcqRel);
        Ok(Attrs {
            id,
            attrs: file(
                "",
                if path.ends_with("untyped-link") {
                    0o120777
                } else {
                    0o100755
                },
            )
            .attrs,
        })
    }
    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, StatusCode> {
        self.config.observed.stats.fetch_add(1, Ordering::AcqRel);
        if self.config.mode == Mode::StallStat {
            std::future::pending::<()>().await;
        }
        if path.ends_with("/broken") {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Attrs {
            id,
            attrs: file(
                "",
                if path.ends_with("/linkdir") {
                    0o040755
                } else {
                    0o100755
                },
            )
            .attrs,
        })
    }
    async fn close(&mut self, id: u32, handle: String) -> Result<Status, StatusCode> {
        assert_eq!(handle, HANDLE);
        self.config.observed.closes.fetch_add(1, Ordering::AcqRel);
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}
struct Handler {
    config: Config,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    jobs: JoinSet<()>,
}
impl Drop for Handler {
    fn drop(&mut self) {
        self.jobs.abort_all();
    }
}
impl server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "ephemeral" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        let index = self.config.observed.opens.fetch_add(1, Ordering::AcqRel);
        if self.config.mode == Mode::LateOpen && index == 0 {
            self.jobs.spawn(async move {
                tokio::time::sleep(Duration::from_millis(200)).await;
                reply.accept().await;
            });
        } else {
            reply.accept().await;
        }
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.config
            .observed
            .execs
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(data.to_vec());
        session.channel_success(id)?;
        if data == PROBE && self.config.mode == Mode::StallProbe {
            return Ok(());
        }
        if data == PROBE {
            session.data(id, self.config.probe.clone())?;
            if !self.config.stderr.is_empty() {
                session.extended_data(id, 1, self.config.stderr.clone())?;
            }
            if let Some(exit) = self.config.exit {
                session.exit_status_request(id, exit)?;
            }
        } else {
            session.data(id, data.to_vec())?;
            session.exit_status_request(id, 0)?;
        }
        session.close(id)?;
        self.channels.remove(&id);
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(name, "sftp");
        session.channel_success(id)?;
        let channel = self.channels.remove(&id).ok_or(russh::Error::Disconnect)?;
        let config = self.config.clone();
        config.observed.active.fetch_add(1, Ordering::AcqRel);
        self.jobs.spawn(async move {
            let filesystem = Filesystem {
                config,
                offset: 0,
                directory: String::new(),
            };
            if matches!(
                filesystem.config.mode,
                Mode::BadUtf8
                    | Mode::BadHandle
                    | Mode::LongHandle
                    | Mode::HugeNameCount
                    | Mode::PoisonStatus
            ) {
                let _ = malformed_names(channel.into_stream(), filesystem).await;
            } else {
                russh_sftp::server::run(channel.into_stream(), filesystem).await;
            }
        });
        Ok(())
    }
    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        self.config
            .observed
            .channel_closes
            .fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
struct Server {
    task: JoinHandle<()>,
    observed: Arc<Observed>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn connect(config: Config) -> TestResult<(SshSession, Server)> {
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let server_config = Arc::new(server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(1),
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let observed = config.observed.clone();
    let task = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await
            && let Ok(running) = server::run_stream(
                server_config,
                stream,
                Handler {
                    config,
                    channels: HashMap::new(),
                    jobs: JoinSet::new(),
                },
            )
            .await
        {
            let _ = running.await;
        }
    });
    let mut options = SshOptions::new("127.0.0.1", "fixture");
    options.port = port;
    options.expected_host_key = Some(fingerprint);
    options.auth = SshAuth::Password(Zeroizing::new("ephemeral".into()));
    options.timeout = Duration::from_secs(2);
    let ssh = SshSession::connect(options).await?;
    Ok((ssh, Server { task, observed }))
}
fn paths(prefix: &str) -> CompletionQuery {
    CompletionQuery::Paths {
        directory: "/fixture".into(),
        prefix: prefix.into(),
        directories_only: false,
    }
}
async fn bounded(future: impl std::future::Future<Output = TestResult>) -> TestResult {
    tokio::time::timeout(Duration::from_secs(12), future).await?
}
async fn wait_count(counter: &AtomicUsize, min: usize) -> TestResult {
    tokio::time::timeout(Duration::from_secs(2), async {
        while counter.load(Ordering::Acquire) < min {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}
async fn wait_closed(server: &Server) -> TestResult {
    tokio::time::timeout(Duration::from_secs(3), async {
        while server.observed.active.load(Ordering::Acquire) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    Ok(())
}

#[tokio::test]
async fn commands_preserve_path_precedence_and_only_query_fixed_probe() -> TestResult {
    bounded(async {
        let (ssh, server) = connect(Config::new(Mode::Normal)).await?;
        let result = ssh
            .complete_remote(CompletionQuery::Commands { prefix: "".into() })
            .await?;
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "linkcmd", "untyped-link", "zeta", "later"]
        );
        assert_eq!(result.candidates[0].path, "/bin/alpha");
        assert!(result.candidates[1].is_symlink);
        assert!(result.candidates[2].is_symlink);
        assert_eq!(result.skipped_path_directories, 4);
        assert!(
            result
                .candidates
                .iter()
                .all(|row| row.kind == CompletionKind::Executable)
        );
        assert_eq!(result.resolved_directory, None);
        assert_eq!(
            *server.observed.execs.lock().map_err(|_| "lock")?,
            vec![PROBE.to_vec()]
        );
        assert_eq!(server.observed.unexpected.load(Ordering::Acquire), 0);
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn paths_use_realpath_and_literal_names_without_shell_execution() -> TestResult {
    bounded(async {
        let (ssh, server) = connect(Config::new(Mode::Unsafe)).await?;
        assert_eq!(ssh.completion_base().await?, "/fixture");
        let result = ssh
            .complete_remote(CompletionQuery::Paths {
                directory: "/literal/$(echo wrong);'\"".into(),
                prefix: "".into(),
                directories_only: false,
            })
            .await?;
        assert_eq!(result.resolved_directory.as_deref(), Some("/fixture"));
        assert_eq!(result.candidates.len(), 2);
        assert_eq!(result.skipped_unsafe_entries, 5);
        assert!(
            result
                .candidates
                .iter()
                .all(|row| row.path == format!("/fixture/{}", row.name))
        );
        assert!(server.observed.execs.lock().map_err(|_| "lock")?.is_empty());
        assert!(
            server
                .observed
                .paths
                .lock()
                .map_err(|_| "lock")?
                .contains(&"/literal/$(echo wrong);'\"".into())
        );
        assert_eq!(server.observed.unexpected.load(Ordering::Acquire), 0);
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn directory_filter_follows_links_and_does_not_return_files() -> TestResult {
    bounded(async {
        let (ssh, _server) = connect(Config::new(Mode::Normal)).await?;
        let result = ssh
            .complete_remote(CompletionQuery::Paths {
                directory: "/fixture".into(),
                prefix: "".into(),
                directories_only: true,
            })
            .await?;
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["directory", "linkdir"]
        );
        assert!(result.candidates[1].is_symlink);
        assert!(
            ssh.complete_remote(paths("ALPHA"))
                .await?
                .candidates
                .is_empty()
        );
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn limits_bound_candidates_scanned_names_and_metadata_requests() -> TestResult {
    bounded(async {
        for (mode, prefix) in [
            (Mode::Many, ""),
            (Mode::ScanLimit, "absent"),
            (Mode::ByteLimit, "absent"),
            (Mode::StatLimit, ""),
        ] {
            let (ssh, server) = connect(Config::new(mode)).await?;
            let result = ssh.complete_remote(paths(prefix)).await?;
            assert!(result.limited);
            assert!(result.candidates.len() <= 64);
            if mode == Mode::Many {
                assert_eq!(result.candidates.len(), 64);
                assert_eq!(result.candidates[0].name, "n00000");
                assert_eq!(result.candidates[63].name, "n00063");
            }
            if mode == Mode::ScanLimit {
                assert!(server.observed.reads.load(Ordering::Acquire) <= 83);
            }
            if mode == Mode::ByteLimit {
                assert!(server.observed.reads.load(Ordering::Acquire) <= 21);
            }
            if mode == Mode::StatLimit {
                assert_eq!(server.observed.stats.load(Ordering::Acquire), 64);
            }
            ssh.close().await?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn malformed_probe_and_peer_diagnostics_never_enter_errors() -> TestResult {
    bounded(async {
        for case in 0..5 {
            let mut config = Config::new(Mode::Normal);
            match case {
                0 => config.probe = b"secret banner".to_vec(),
                1 => config.exit = None,
                2 => config.exit = Some(1),
                3 => config.stderr = b"secret stderr\x1b[2J".to_vec(),
                _ => config.probe = vec![b'x'; 16 * 1024 + 1],
            }
            let (ssh, _server) = connect(config).await?;
            let result = ssh
                .complete_remote(CompletionQuery::Commands { prefix: "".into() })
                .await;
            assert!(result.is_err());
            let text = format!("{result:?}");
            assert!(!text.contains("secret"));
            assert!(!text.contains('\x1b'));
            ssh.close().await?;
        }
        for mode in [
            Mode::EmptyPage,
            Mode::BadCanonical,
            Mode::Denied,
            Mode::HugeNameCount,
            Mode::PoisonStatus,
        ] {
            let (ssh, _server) = connect(Config::new(mode)).await?;
            assert!(ssh.complete_remote(paths("")).await.is_err());
            ssh.close().await?;
        }
        Ok(())
    })
    .await
}

#[tokio::test]
async fn lossy_wire_filename_is_never_returned_as_a_different_path() -> TestResult {
    bounded(async {
        let (ssh, server) = connect(Config::new(Mode::BadUtf8)).await?;
        let result = ssh.complete_remote(paths("")).await?;
        assert!(result.candidates.is_empty());
        assert_eq!(result.skipped_unsafe_entries, 1);
        wait_closed(&server).await?;
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn cancelled_read_and_stat_close_owned_subsystems_but_keep_shared_ssh() -> TestResult {
    bounded(async{
    for mode in [Mode::StallRead,Mode::StallStat,Mode::StallInit] {
        let(ssh,server)=connect(Config::new(mode)).await?;
        let mut request=Box::pin(ssh.complete_remote(paths("")));
        tokio::select! { result=&mut request=>return Err(format!("unexpected completion {result:?}").into()), observed=async{
            match mode {Mode::StallRead=>wait_count(&server.observed.reads,1).await,Mode::StallStat=>wait_count(&server.observed.stats,1).await,_=>wait_count(&server.observed.active,1).await}
        }=>{observed?;} }
        drop(request);
        wait_count(&server.observed.channel_closes, 1).await?;
        // A stalled server handler can retain its own future after EOF. The SSH
        // channel still closes independently; unrelated exec must remain usable.
        assert_eq!(ssh.exec("survives").await?.stdout,b"survives");assert!(!ssh.is_closed());ssh.close().await?;
    }Ok(())
}).await
}

#[tokio::test]
async fn stalled_query_uses_a_whole_operation_deadline() -> TestResult {
    bounded(async {
        let (ssh, _server) = connect(Config::new(Mode::StallRead)).await?;
        let started = tokio::time::Instant::now();
        assert_eq!(
            ssh.complete_remote(paths("")).await,
            Err(CompletionError::Timeout)
        );
        assert!(started.elapsed() < Duration::from_secs(7));
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

async fn malformed_names(
    mut stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    filesystem: Filesystem,
) -> TestResult {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn string(out: &mut Vec<u8>, value: &[u8]) {
        out.extend_from_slice(&(value.len() as u32).to_be_bytes());
        out.extend_from_slice(value);
    }
    let mut read = false;
    while let Ok(len) = stream.read_u32().await {
        if len > 65536 {
            return Err("fixture packet too large".into());
        }
        let mut packet = vec![0; len as usize];
        stream.read_exact(&mut packet).await?;
        let id = packet.get(1..5).ok_or("short request")?;
        let mut response = Vec::new();
        match packet[0] {
            1 => {
                response.push(2);
                response.extend_from_slice(&3_u32.to_be_bytes());
            }
            16 if filesystem.config.mode == Mode::PoisonStatus => {
                response.push(101);
                response.extend_from_slice(id);
                response.extend_from_slice(&3_u32.to_be_bytes());
                string(&mut response, b"peer-secret\x1b[2J");
                string(&mut response, b"en");
            }
            16 => {
                response.push(104);
                response.extend_from_slice(id);
                response.extend_from_slice(&1_u32.to_be_bytes());
                string(&mut response, b"/fixture");
                string(&mut response, b"");
                response.extend_from_slice(&0_u32.to_be_bytes());
            }
            11 => {
                response.push(102);
                response.extend_from_slice(id);
                let handle = match filesystem.config.mode {
                    Mode::BadHandle => vec![0xff],
                    Mode::LongHandle => vec![0; 257],
                    _ => HANDLE.as_bytes().to_vec(),
                };
                string(&mut response, &handle);
            }
            12 if !read => {
                read = true;
                response.push(104);
                response.extend_from_slice(id);
                response.extend_from_slice(
                    &(if filesystem.config.mode == Mode::HugeNameCount {
                        u32::MAX
                    } else {
                        1
                    })
                    .to_be_bytes(),
                );
                if filesystem.config.mode != Mode::HugeNameCount {
                    string(&mut response, b"bad-\xff");
                    string(&mut response, b"");
                    response.extend_from_slice(&4_u32.to_be_bytes());
                    response.extend_from_slice(&0o100644_u32.to_be_bytes());
                }
            }
            kind => {
                response.push(101);
                response.extend_from_slice(id);
                response.extend_from_slice(&(if kind == 4 { 0_u32 } else { 1_u32 }).to_be_bytes());
                string(&mut response, b"");
                string(&mut response, b"");
            }
        }
        stream.write_u32(response.len() as u32).await?;
        stream.write_all(&response).await?;
    }
    Ok(())
}

#[tokio::test]
async fn exact_scan_boundary_marks_omitted_later_path_directories() -> TestResult {
    bounded(async {
        let (ssh, _server) = connect(Config::new(Mode::ExactScan)).await?;
        let result = ssh
            .complete_remote(CompletionQuery::Commands {
                prefix: "absent".into(),
            })
            .await?;
        assert!(result.limited);
        assert!(result.candidates.is_empty());
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn cancelled_probe_and_late_open_are_owned_without_closing_shared_transport() -> TestResult {
    bounded(async {
        for mode in [Mode::StallProbe, Mode::LateOpen] {
            let (ssh, server) = connect(Config::new(mode)).await?;
            let mut request = Box::pin(ssh.complete_remote(CompletionQuery::Commands { prefix: "".into() }));
            tokio::select! {
                result = &mut request => return Err(format!("unexpected completion {result:?}").into()),
                observed = async {
                    wait_count(&server.observed.opens, 1).await?;
                    if mode == Mode::StallProbe {
                        tokio::time::timeout(Duration::from_secs(2), async {
                            loop {
                                if !server.observed.execs.lock().map_err(|_| "fixture lock")?.is_empty() { break; }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                            Ok::<_, Box<dyn Error + Send + Sync>>(())
                        }).await??;
                    }
                    Ok::<_, Box<dyn Error + Send + Sync>>(())
                } => observed?,
            }
            drop(request);
            wait_count(&server.observed.channel_closes, 1).await?;
            assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
            assert!(!ssh.is_closed());
            ssh.close().await?;
        }
        Ok(())
    }).await
}

#[tokio::test]
async fn initialization_and_each_metadata_request_share_the_same_deadline() -> TestResult {
    bounded(async {
        let (ssh, _server) = connect(Config::new(Mode::SlowSteps)).await?;
        let started = tokio::time::Instant::now();
        assert_eq!(
            ssh.complete_remote(paths("")).await,
            Err(CompletionError::Timeout)
        );
        // Each of init/realpath/readdir takes 1.8s; resetting per request would
        // finish only after the subsequent EOF read, beyond seven seconds.
        assert!(started.elapsed() >= Duration::from_millis(4500));
        assert!(started.elapsed() < Duration::from_secs(7));
        assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn peer_status_message_is_discarded_before_becoming_a_public_error() -> TestResult {
    bounded(async {
        let (ssh, _server) = connect(Config::new(Mode::PoisonStatus)).await?;
        let result = ssh.complete_remote(paths("")).await;
        assert_eq!(result, Err(CompletionError::PermissionDenied));
        assert!(!format!("{result:?}").contains("peer-secret"));
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn opaque_handles_that_cannot_round_trip_exactly_are_rejected() -> TestResult {
    bounded(async {
        for mode in [Mode::BadHandle, Mode::LongHandle] {
            let (ssh, server) = connect(Config::new(mode)).await?;
            assert_eq!(
                ssh.complete_remote(paths("")).await,
                Err(CompletionError::InvalidResponse)
            );
            wait_count(&server.observed.channel_closes, 1).await?;
            assert_eq!(ssh.exec("survives").await?.stdout, b"survives");
            ssh.close().await?;
        }
        let (ssh, _server) = connect(Config::new(Mode::Normal)).await?;
        assert_eq!(
            ssh.complete_remote(CompletionQuery::Paths {
                directory: "//fixture".into(),
                prefix: "".into(),
                directories_only: false
            })
            .await,
            Err(CompletionError::InvalidResponse)
        );
        ssh.close().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn exact_name_byte_boundary_marks_omitted_later_path_directories() -> TestResult {
    bounded(async {
        let (ssh, server) = connect(Config::new(Mode::ExactBytes)).await?;
        let result = ssh
            .complete_remote(CompletionQuery::Commands {
                prefix: "absent".into(),
            })
            .await?;
        assert!(result.limited);
        assert!(result.candidates.is_empty());
        assert_eq!(
            *server.observed.paths.lock().map_err(|_| "fixture lock")?,
            ["/bin"]
        );
        assert_eq!(server.observed.reads.load(Ordering::Acquire), 42);
        ssh.close().await?;
        Ok(())
    })
    .await
}
