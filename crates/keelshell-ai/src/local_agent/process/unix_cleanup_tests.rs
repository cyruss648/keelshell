//! Actual owned Unix groups exercise the same private helper used by cleanup.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    future::{Future, poll_fn},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
            net::UnixListener,
        },
    },
    path::{Path, PathBuf},
    task::Poll,
    time::Instant as StdInstant,
};

use nix::{
    errno::Errno,
    fcntl::{FcntlArg, FdFlag, fcntl},
    sys::{
        signal::killpg,
        socket::{AddressFamily, SockFlag, SockProtocol, SockType, UnixAddr, connect, socket},
    },
    unistd::{getpgid, getpid},
};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncReadExt, net::UnixStream};
use uuid::Uuid;

use super::*;

const ENTRYPOINT: &str = "local_agent::process::unix_cleanup_tests::owned_group_fixture_entrypoint";
const FIXTURE_ARGUMENTS: [&str; 6] = [
    "--exact",
    ENTRYPOINT,
    "--ignored",
    "--test-threads=1",
    "--nocapture",
    "--color=never",
];
const MODE_ENV: &str = "KEELSHELL_OWNED_UNIX_GROUP_MODE";
const ROOT_ENV: &str = "KEELSHELL_OWNED_UNIX_GROUP_ROOT";
const NONCE_ENV: &str = "KEELSHELL_OWNED_UNIX_GROUP_NONCE";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct MemberIdentity {
    pid: u32,
    group: i32,
    nonce: Uuid,
}

fn private_fixture_log(root: &Path, name: &str) -> std::fs::File {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(root.join(name))
        .unwrap()
}

// An observed held-leader fixture failed while setting SO_RCVTIMEO.
// Avoid that optional setting and read the complete nonce under one 100ms
// deadline capped by the leader's existing hard lifetime. Partial reads and
// transient errors cannot restart either budget or authorize a partial nonce.
fn read_held_leader_request(
    stream: &mut std::os::unix::net::UnixStream,
    leader_deadline: StdInstant,
) -> std::io::Result<[u8; 17]> {
    let deadline = (StdInstant::now() + Duration::from_millis(100)).min(leader_deadline);
    stream.set_nonblocking(true)?;
    let mut request = [0; 17];
    let mut offset = 0;
    loop {
        if StdInstant::now() >= deadline {
            return Err(std::io::Error::new(
                ErrorKind::TimedOut,
                "held fixture request deadline expired",
            ));
        }
        match stream.read(&mut request[offset..]) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "held fixture request ended before its complete nonce",
                ));
            }
            Ok(bytes) => {
                offset += bytes;
                if offset == request.len() {
                    if StdInstant::now() >= deadline {
                        return Err(std::io::Error::new(
                            ErrorKind::TimedOut,
                            "held fixture request arrived after its deadline",
                        ));
                    }
                    return Ok(request);
                }
            }
            Err(error)
                if matches!(error.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) =>
            {
                let remaining = deadline.saturating_duration_since(StdInstant::now());
                std::thread::sleep(Duration::from_millis(2).min(remaining));
            }
            Err(error) => return Err(error),
        }
    }
}

// This ignored entry is invoked only by the actual owned test executable with
// fixed argv and a private fixture root. It never invokes a supplier CLI.
#[test]
#[ignore = "self-hosted Unix group fixture, invoked by owned cleanup controls"]
#[allow(
    clippy::zombie_processes,
    reason = "intentional contained descendant after leader exit; group owner stops it"
)]
fn owned_group_fixture_entrypoint() {
    let root = PathBuf::from(std::env::var_os(ROOT_ENV).unwrap());
    let nonce = Uuid::parse_str(&std::env::var(NONCE_ENV).unwrap()).unwrap();
    match std::env::var(MODE_ENV).unwrap().as_str() {
        mode @ ("leader" | "held-leader" | "long-tmp-leader" | "long-tmp-held-leader") => {
            let held = matches!(mode, "held-leader" | "long-tmp-held-leader");
            let long_tmp = matches!(mode, "long-tmp-leader" | "long-tmp-held-leader");
            if long_tmp {
                assert!(std::env::temp_dir().as_os_str().as_bytes().len() > 103);
            }
            let leader_listener = held.then(|| {
                let listener = UnixListener::bind(root.join("leader.sock")).unwrap();
                listener.set_nonblocking(true).unwrap();
                listener
            });
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(FIXTURE_ARGUMENTS);
            command
                .env_clear()
                .env(
                    MODE_ENV,
                    if long_tmp {
                        "long-tmp-member"
                    } else {
                        "member"
                    },
                )
                .env(ROOT_ENV, &root)
                .env(NONCE_ENV, nonce.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(private_fixture_log(&root, "member-stderr.log"));
            if long_tmp {
                command.env("TMPDIR", std::env::var_os("TMPDIR").unwrap());
            }
            let member = command.spawn().unwrap();
            let deadline = StdInstant::now() + Duration::from_secs(2);
            loop {
                if let Ok(bytes) = std::fs::read(root.join("member-ready.json"))
                    && let Ok(identity) = serde_json::from_slice::<MemberIdentity>(&bytes)
                {
                    assert_eq!(identity.pid, member.id());
                    assert_eq!(identity.nonce, nonce);
                    assert_eq!(identity.group, getpgid(None).unwrap().as_raw());
                    break;
                }
                assert!(
                    StdInstant::now() < deadline,
                    "owned member readiness expired"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
            // Dropping std::process::Child does not wait or kill. The actual
            // group cleanup tests, rather than this fixture leader, own stop.
            drop(member);
            if let Some(listener) = leader_listener {
                let deadline = StdInstant::now() + Duration::from_secs(10);
                while StdInstant::now() < deadline {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let request = read_held_leader_request(&mut stream, deadline).unwrap();
                            if request[0] == b'Q' && request[1..] == *nonce.as_bytes() {
                                return;
                            }
                        }
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("owned leader accept failed: {error}"),
                    }
                }
            }
        }
        mode @ ("member" | "long-tmp-member") => {
            if mode == "long-tmp-member" {
                assert!(std::env::temp_dir().as_os_str().as_bytes().len() > 103);
            }
            let listener = UnixListener::bind(root.join("member.sock")).unwrap();
            listener.set_nonblocking(true).unwrap();
            let identity = MemberIdentity {
                pid: u32::try_from(getpid().as_raw()).unwrap(),
                group: getpgid(None).unwrap().as_raw(),
                nonce,
            };
            let temporary = root.join("member-ready.partial");
            let mut ready = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&temporary)
                .unwrap();
            serde_json::to_writer(&mut ready, &identity).unwrap();
            ready.flush().unwrap();
            drop(ready);
            std::fs::rename(temporary, root.join("member-ready.json")).unwrap();
            let deadline = StdInstant::now() + Duration::from_secs(10);
            while StdInstant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_millis(100)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_millis(100)))
                            .unwrap();
                        let mut request = [0];
                        if stream.read_exact(&mut request).is_ok() {
                            match request[0] {
                                b'I' => {
                                    let _ = serde_json::to_writer(&mut stream, &identity);
                                }
                                b'Q' => {
                                    let mut supplied_nonce = [0; 16];
                                    if stream.read_exact(&mut supplied_nonce).is_ok()
                                        && supplied_nonce == *nonce.as_bytes()
                                    {
                                        let _ = serde_json::to_writer(&mut stream, &identity);
                                        return;
                                    }
                                }
                                _ => panic!("unexpected owned fixture request"),
                            }
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("owned fixture accept failed: {error}"),
                }
            }
        }
        _ => panic!("unexpected owned fixture mode"),
    }
}

// Socket path capacity counts every inherited TMPDIR component. Use a short
// generic Unix namespace with a random 0700 owner directory, then validate
// both complete endpoint names through the actual native-address constructor.
// This root is outside the check controller's TMPDIR; its own close/retention
// receipt, rather than an empty outer scratch, establishes resource state.
fn private_socket_root() -> std::io::Result<TempDir> {
    let root = tempfile::Builder::new()
        .prefix("ks-ug-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in("/tmp")?;
    for endpoint in ["member.sock", "leader.sock"] {
        UnixAddr::new(&root.path().join(endpoint)).map_err(std::io::Error::from)?;
    }
    Ok(root)
}

struct GroupFixture {
    owned: OwnedChild,
    root: Option<TempDir>,
    group: UnixProcessGroup,
    identity: Option<MemberIdentity>,
    nonce: Uuid,
    deadline: Instant,
}

impl GroupFixture {
    async fn start() -> Self {
        Self::start_with_held_leader(false).await
    }

    async fn start_with_held_leader(held: bool) -> Self {
        Self::start_with_tmpdir(held, None).await
    }

    async fn start_with_tmpdir(held: bool, child_tmpdir: Option<&Path>) -> Self {
        let root = private_socket_root().unwrap();
        let nonce = Uuid::new_v4();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(FIXTURE_ARGUMENTS);
        command
            .env_clear()
            .env(
                MODE_ENV,
                match (held, child_tmpdir.is_some()) {
                    (false, false) => "leader",
                    (true, false) => "held-leader",
                    (false, true) => "long-tmp-leader",
                    (true, true) => "long-tmp-held-leader",
                },
            )
            .env(ROOT_ENV, root.path())
            .env(NONCE_ENV, nonce.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(private_fixture_log(root.path(), "leader-stderr.log"));
        if let Some(tmpdir) = child_tmpdir {
            command.env("TMPDIR", tmpdir);
        }
        let owned = OwnedChild::spawn(command).unwrap();
        let leader = owned.child.id().unwrap();
        let group = owned.observed_group.unwrap();
        let mut fixture = Self {
            owned,
            root: Some(root),
            group,
            identity: None,
            nonce,
            deadline: Instant::now() + Duration::from_secs(8),
        };
        let ready_deadline = (Instant::now() + Duration::from_secs(2)).min(fixture.deadline);
        loop {
            if let Ok(bytes) = std::fs::read(fixture.path().join("member-ready.json"))
                && let Ok(identity) = serde_json::from_slice::<MemberIdentity>(&bytes)
            {
                assert_eq!(identity.nonce, nonce);
                assert_eq!(identity.group, group.0.as_raw());
                assert_eq!(
                    getpgid(Some(nix::unistd::Pid::from_raw(
                        i32::try_from(identity.pid).unwrap()
                    )))
                    .unwrap(),
                    group.0
                );
                fixture.identity = Some(identity);
                break;
            }
            assert!(
                Instant::now() < ready_deadline,
                "owned group readiness expired"
            );
            tokio::time::sleep_until(
                (Instant::now() + Duration::from_millis(2)).min(ready_deadline),
            )
            .await;
        }
        if !held {
            let status =
                tokio::time::timeout_at(fixture.deadline, fixture.owned.child.inner_mut().wait())
                    .await
                    .unwrap()
                    .unwrap();
            assert!(status.success(), "owned leader actual exit: {status}");
            assert!(
                fixture.owned.child.id().is_none(),
                "native leader was not reaped"
            );
            eprintln!(
                "unix-group-control {}",
                serde_json::json!({"phase":"leader_reaped","leader_pid":leader,"member_pid":fixture.identity.as_ref().unwrap().pid,"saved_group":group.0.as_raw(),"actual_wait_code":status.code(),"native_id_absent":true,"socket_root":fixture.path(),"socket_namespace":"private_short_root_outside_outer_tmp"})
            );
        } else {
            assert_eq!(fixture.owned.child.id(), Some(leader));
        }
        fixture.assert_member_alive().await;
        fixture
    }

    fn path(&self) -> &Path {
        self.root.as_ref().unwrap().path()
    }

    async fn request(&self, byte: u8) -> Vec<u8> {
        tokio::time::timeout_at(self.deadline, async {
            let mut stream = UnixStream::connect(self.path().join("member.sock"))
                .await
                .unwrap();
            stream.write_all(&[byte]).await.unwrap();
            if byte == b'Q' {
                stream
                    .write_all(self.identity.as_ref().unwrap().nonce.as_bytes())
                    .await
                    .unwrap();
            }
            stream.shutdown().await.unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).await.unwrap();
            assert!(
                response.len() <= 256,
                "owned identity response exceeded bound"
            );
            response
        })
        .await
        .unwrap()
    }

    async fn assert_member_alive(&self) {
        let identity: MemberIdentity = serde_json::from_slice(&self.request(b'I').await).unwrap();
        assert_eq!(Some(&identity), self.identity.as_ref());
        assert_eq!(killpg(self.group.0, None), Ok(()));
    }

    async fn finish(mut self) {
        self.request(b'Q').await;
        self.group
            .wait_until_absent((Instant::now() + CLEANUP_DEADLINE).min(self.deadline))
            .await
            .unwrap();
        assert_eq!(killpg(self.group.0, None), Err(Errno::ESRCH));
        self.close_root();
    }

    async fn release_leader(&self) {
        tokio::time::timeout_at(self.deadline, async {
            let mut stream = UnixStream::connect(self.path().join("leader.sock"))
                .await
                .unwrap();
            stream.write_all(b"Q").await.unwrap();
            stream
                .write_all(self.identity.as_ref().unwrap().nonce.as_bytes())
                .await
                .unwrap();
            stream.shutdown().await.unwrap();
        })
        .await
        .unwrap();
    }

    async fn stop_member_and_close_root(&mut self) {
        let returned: MemberIdentity = serde_json::from_slice(&self.request(b'Q').await).unwrap();
        assert_eq!(Some(&returned), self.identity.as_ref());
        self.group
            .wait_until_absent((Instant::now() + CLEANUP_DEADLINE).min(self.deadline))
            .await
            .unwrap();
        assert!(self.owned.child.id().is_none());
        self.close_root();
    }

    fn close_root(&mut self) {
        assert_eq!(killpg(self.group.0, None), Err(Errno::ESRCH));
        assert!(self.owned.child.id().is_none());
        // A helper-only control stopped its own fixture by authenticated IPC.
        // Do not subsequently exercise a first signal on an already absent
        // numeric group, and do not rewrite it as production cleanup success.
        if matches!(self.owned.unix_cleanup, UnixCleanupState::Ready) {
            self.owned.unix_cleanup = UnixCleanupState::Failed;
        }
        let root = self.root.take().unwrap();
        let path = root.path().to_owned();
        root.close().unwrap();
        assert!(!path.exists());
        eprintln!(
            "unix-group-control {}",
            serde_json::json!({"phase":"complete","saved_group":self.group.0.as_raw(),"group_absent":true,"socket_root":path,"scratch_removed":true,"outer_tmp_proves_socket_cleanup":false})
        );
    }
}

// Drop cannot await Tokio and a blocking connect has no deadline API. Use a
// nonblocking socket and one bounded write to the known private fixture path;
// the outer fallback retries only while its absolute deadline remains live.
fn try_stop_fixture(root: &Path, endpoint: &str, nonce: Uuid) {
    let path = root.join(endpoint);
    let Ok(address) = UnixAddr::new(&path) else {
        return;
    };
    // Empty flags work on macOS, where nix does not export SOCK_CLOEXEC or
    // SOCK_NONBLOCK. OwnedFd closes every early-return path without raw-fd
    // ownership conversion; configure it before connecting or sending data.
    let Ok(descriptor) = socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::empty(),
        None::<SockProtocol>,
    ) else {
        return;
    };
    if fcntl(&descriptor, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)).is_err() {
        return;
    }
    let mut stream = std::os::unix::net::UnixStream::from(descriptor);
    if stream.set_nonblocking(true).is_err() || connect(stream.as_raw_fd(), &address).is_err() {
        return;
    }
    let mut request = [0; 17];
    request[0] = b'Q';
    request[1..].copy_from_slice(nonce.as_bytes());
    // One nonblocking write: partial/error is retried by the outer deadline,
    // never an unbounded write_all/EINTR loop inside Drop.
    let _ = stream.write(&request);
}

impl Drop for GroupFixture {
    fn drop(&mut self) {
        let Some(root) = self.root.take() else {
            return;
        };
        // Failed controls retain private stderr. Fallback never signals a
        // saved numeric PGID: it addresses only nonce-protected fixture IPC.
        let root = root.keep();
        // Disarm only fixture-owner fallback permission, never assert success.
        // Leader and member also have hard lifetimes if IPC is unavailable.
        if matches!(self.owned.unix_cleanup, UnixCleanupState::Ready) {
            self.owned.unix_cleanup = UnixCleanupState::Failed;
        }
        let deadline = StdInstant::now() + CLEANUP_DEADLINE;
        loop {
            if StdInstant::now() < deadline {
                for endpoint in ["leader.sock", "member.sock"] {
                    try_stop_fixture(&root, endpoint, self.nonce);
                }
            }
            let reaped = self
                .owned
                .child
                .inner_mut()
                .try_wait()
                .is_ok_and(|status| status.is_some());
            let absent = killpg(self.group.0, None) == Err(Errno::ESRCH);
            if reaped && absent {
                eprintln!(
                    "unix-group-control {}",
                    serde_json::json!({"phase":"fallback","saved_group":self.group.0.as_raw(),"leader_reaped":true,"group_absent":true,"failed_scratch_retained":true,"socket_root":root,"outer_tmp_proves_socket_cleanup":false,"cleanup_claimed_success":self.owned.cleaned})
                );
                return;
            }
            if StdInstant::now() >= deadline {
                eprintln!(
                    "unix-group-control {}",
                    serde_json::json!({"phase":"fallback_terminal_unknown","socket_root":root,"private_root_retained":true,"outer_tmp_proves_socket_cleanup":false})
                );
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

#[tokio::test]
async fn reaped_leader_does_not_finish_live_group_and_natural_member_exit_does() {
    let fixture = GroupFixture::start().await;
    let mut waiting = Box::pin(
        fixture
            .group
            .wait_until_absent((Instant::now() + CLEANUP_DEADLINE).min(fixture.deadline)),
    );
    let first = poll_fn(|context| Poll::Ready(waiting.as_mut().poll(context))).await;
    assert!(
        first.is_pending(),
        "a live original member cannot satisfy cleanup"
    );
    fixture.assert_member_alive().await;
    fixture.request(b'Q').await;
    waiting.as_mut().await.unwrap();
    assert_eq!(killpg(fixture.group.0, None), Err(Errno::ESRCH));
    drop(waiting);
    // Direct helper completion is not inherited as owner cleanup success.
    let mut fixture = fixture;
    fixture.close_root();
}

#[tokio::test]
async fn live_original_group_reaches_same_deadline_as_typed_cleanup_failure() {
    let fixture = GroupFixture::start().await;
    let deadline = (Instant::now() + Duration::from_millis(30)).min(fixture.deadline);
    assert_eq!(
        fixture.group.wait_until_absent(deadline).await,
        Err(LocalAgentError::CleanupFailed)
    );
    fixture.assert_member_alive().await;
    fixture.finish().await;
}

#[tokio::test]
async fn group_observation_other_unknown_errors_fail_immediately_without_signal() {
    let fixture = GroupFixture::start().await;
    for error in [Errno::EIO, Errno::EINVAL, Errno::ECHILD] {
        let mut observations = 0;
        let result = fixture
            .group
            .wait_until_absent_observed(fixture.deadline, |group| {
                assert_eq!(group, fixture.group.0);
                observations += 1;
                Err(error)
            })
            .await;
        assert_eq!(result, Err(LocalAgentError::CleanupFailed));
        assert_eq!(
            observations, 1,
            "an unknown observation must not retry or act"
        );
        fixture.assert_member_alive().await;
    }
    fixture.finish().await;
}

#[tokio::test]
async fn transient_permission_observation_requires_actual_absence_and_never_resignals() {
    let mut fixture = GroupFixture::start().await;
    let group = fixture.group;
    let attempts = fixture.owned.cleanup_signal_attempts.clone();
    let mut unknown_reads = 0;
    let mut cleanup = Box::pin(fixture.owned.cleanup_unix_with(
        |_| Ok(()),
        |id| {
            assert_eq!(id, group.0);
            assert_eq!(killpg(id, None), Ok(()));
            unknown_reads += 1;
            Err(Errno::EPERM)
        },
    ));
    assert!(
        poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx)))
            .await
            .is_pending(),
        "permission denied cannot establish cleanup success or immediate failure"
    );
    drop(cleanup);
    assert_eq!(unknown_reads, 1);
    let UnixCleanupState::Observing { deadline } = fixture.owned.unix_cleanup else {
        panic!("unknown observation must retain the original observing state");
    };
    assert!(fixture.owned.child.id().is_none());
    assert!(!fixture.owned.cleaned);
    fixture.assert_member_alive().await;
    let returned: MemberIdentity = serde_json::from_slice(&fixture.request(b'Q').await).unwrap();
    assert_eq!(Some(&returned), fixture.identity.as_ref());
    let mut native_absence_observed_at = None;
    fixture
        .owned
        .cleanup_unix_with(
            |_| panic!("unknown observation reentry must not terminate again"),
            |id| {
                assert_eq!(id, group.0);
                let result = killpg(id, None);
                if result == Err(Errno::ESRCH) {
                    native_absence_observed_at = Some(Instant::now());
                }
                result
            },
        )
        .await
        .unwrap();
    assert!(
        native_absence_observed_at.is_some_and(|observed_at| observed_at < deadline),
        "success requires actual native ESRCH before the original deadline"
    );
    assert!(fixture.owned.cleaned);
    assert!(matches!(
        fixture.owned.unix_cleanup,
        UnixCleanupState::Complete
    ));
    assert_eq!(killpg(group.0, None), Err(Errno::ESRCH));
    fixture.close_root();
    drop(fixture);
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn persistent_permission_observation_uses_original_deadline_on_reentry_and_drop() {
    let mut fixture = GroupFixture::start().await;
    let attempts = fixture.owned.cleanup_signal_attempts.clone();
    let mut reads = 0;
    let started = Instant::now();
    let mut cleanup = Box::pin(fixture.owned.cleanup_unix_with(
        |_| Ok(()),
        |_| {
            reads += 1;
            Err(Errno::EPERM)
        },
    ));
    assert!(
        poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(cleanup);
    assert_eq!(reads, 1);
    let UnixCleanupState::Observing { deadline } = fixture.owned.unix_cleanup else {
        panic!("persistent unknown observation must remain pending");
    };
    assert!(deadline >= started + CLEANUP_DEADLINE);
    assert!(!fixture.owned.cleaned);
    fixture.assert_member_alive().await;
    let mut resumed = Box::pin(fixture.owned.cleanup_unix_with(
        |_| panic!("permission denied may not grant another termination"),
        |_| {
            reads += 1;
            Err(Errno::EPERM)
        },
    ));
    assert!(
        poll_fn(|cx| Poll::Ready(resumed.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(resumed);
    assert!(
        matches!(fixture.owned.unix_cleanup, UnixCleanupState::Observing { deadline: same } if same == deadline)
    );
    assert_eq!(
        fixture
            .owned
            .cleanup_unix_with(
                |_| panic!("repeated unknown observation must not terminate"),
                |_| {
                    reads += 1;
                    Err(Errno::EPERM)
                },
            )
            .await,
        Err(LocalAgentError::CleanupFailed)
    );
    assert!(
        reads > 2,
        "EPERM must stay unknown until the original deadline"
    );
    assert!(Instant::now() >= deadline);
    assert!(matches!(
        fixture.owned.unix_cleanup,
        UnixCleanupState::Failed
    ));
    assert!(!fixture.owned.cleaned);
    assert_eq!(
        fixture
            .owned
            .cleanup_unix_with(
                |_| panic!("expired permission failure cannot terminate again"),
                |_| panic!("expired permission failure cannot obtain a new observation"),
            )
            .await,
        Err(LocalAgentError::CleanupFailed)
    );
    fixture.assert_member_alive().await;
    fixture.stop_member_and_close_root().await;
    drop(fixture);
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn late_absence_after_permission_unknown_is_rejected_at_original_deadline() {
    let fixture = GroupFixture::start().await;
    let deadline = (Instant::now() + Duration::from_millis(50)).min(fixture.deadline);
    let mut observations = 0;
    assert_eq!(
        fixture
            .group
            .wait_until_absent_observed(deadline, |group| {
                assert_eq!(group, fixture.group.0);
                assert_eq!(killpg(group, None), Ok(()));
                observations += 1;
                if observations == 1 {
                    Err(Errno::EPERM)
                } else {
                    // A bounded test-only slow observer returns synthetic ESRCH
                    // after the deadline. Presence is still confirmed natively;
                    // this controls admission of a late result, not OS absence.
                    std::thread::sleep(
                        deadline.saturating_duration_since(Instant::now())
                            + Duration::from_millis(1),
                    );
                    Err(Errno::ESRCH)
                }
            })
            .await,
        Err(LocalAgentError::CleanupFailed)
    );
    assert_eq!(observations, 2);
    assert!(Instant::now() >= deadline);
    fixture.assert_member_alive().await;
    fixture.finish().await;
}

#[tokio::test]
async fn cancelled_permission_observation_drop_cannot_resignal_numeric_identity() {
    let mut fixture = GroupFixture::start().await;
    let attempts = fixture.owned.cleanup_signal_attempts.clone();
    let mut cleanup = Box::pin(
        fixture
            .owned
            .cleanup_unix_with(|_| Ok(()), |_| Err(Errno::EPERM)),
    );
    assert!(
        poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(cleanup);
    assert!(matches!(
        fixture.owned.unix_cleanup,
        UnixCleanupState::Observing { .. }
    ));
    assert!(!fixture.owned.cleaned);
    fixture.assert_member_alive().await;
    fixture.stop_member_and_close_root().await;
    drop(fixture);
    assert_eq!(
        attempts.load(Ordering::Relaxed),
        1,
        "OwnedChild Drop must not grant another numeric-group termination"
    );
}

#[tokio::test]
async fn expired_group_deadline_refuses_without_observing_or_signalling() {
    let fixture = GroupFixture::start().await;
    let result = fixture
        .group
        .wait_until_absent_observed(Instant::now(), |_| {
            panic!("expired cleanup may not begin an observation")
        })
        .await;
    assert_eq!(result, Err(LocalAgentError::CleanupFailed));
    fixture.assert_member_alive().await;
    fixture.finish().await;
}

#[tokio::test]
async fn owned_cleanup_confirms_saved_group_after_actual_leader_reap() {
    let mut fixture = GroupFixture::start().await;
    fixture.owned.cleanup().await.unwrap();
    assert!(fixture.owned.cleaned);
    assert_eq!(killpg(fixture.group.0, None), Err(Errno::ESRCH));
    assert!(fixture.owned.child.id().is_none());
    assert!(
        UnixStream::connect(fixture.path().join("member.sock"))
            .await
            .is_err()
    );
    fixture.close_root();
}

#[tokio::test]
async fn identity_capture_refuses_real_native_child_without_installed_group_wrapper() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--list")
        .env_clear()
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command.spawn().unwrap();
    assert!(matches!(
        UnixProcessGroup::capture(&child),
        Err(LocalAgentError::SpawnFailed)
    ));
    let status = tokio::time::timeout(Duration::from_secs(2), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
    assert!(child.id().is_none());
}

#[tokio::test]
async fn cancelled_observation_reentry_preserves_deadline_and_never_resignals() {
    let mut fixture = GroupFixture::start().await;
    let group = fixture.group;
    let attempts = fixture.owned.cleanup_signal_attempts.clone();
    let mut cleanup = Box::pin(
        fixture
            .owned
            .cleanup_unix_with(|_| Ok(()), |id| killpg(id, None)),
    );
    assert!(
        poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(cleanup);
    let UnixCleanupState::Observing { deadline } = fixture.owned.unix_cleanup else {
        panic!("actual wrapper wait must reach observation");
    };
    assert!(!fixture.owned.cleaned);
    fixture.assert_member_alive().await;
    let mut reentered = Box::pin(
        fixture
            .owned
            .cleanup_unix_with(|_| panic!("reentry may not signal"), |id| killpg(id, None)),
    );
    assert!(
        poll_fn(|cx| Poll::Ready(reentered.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(reentered);
    assert!(
        matches!(fixture.owned.unix_cleanup, UnixCleanupState::Observing { deadline: same } if same == deadline)
    );
    let identity: MemberIdentity = serde_json::from_slice(&fixture.request(b'Q').await).unwrap();
    assert_eq!(Some(&identity), fixture.identity.as_ref());
    let mut reads = 0;
    fixture
        .owned
        .cleanup_unix_with(
            |_| panic!("cancelled cleanup may not signal again"),
            |id| {
                assert_eq!(id, group.0);
                reads += 1;
                killpg(id, None)
            },
        )
        .await
        .unwrap();
    assert!(reads > 0);
    assert!(Instant::now() <= deadline);
    assert!(fixture.owned.cleaned);
    assert!(matches!(
        fixture.owned.unix_cleanup,
        UnixCleanupState::Complete
    ));
    fixture.close_root();
    drop(fixture);
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn cancelled_actual_leader_wait_reentry_never_signals_again() {
    let mut fixture = GroupFixture::start_with_held_leader(true).await;
    let attempts = fixture.owned.cleanup_signal_attempts.clone();
    let mut cleanup = Box::pin(
        fixture
            .owned
            .cleanup_unix_with(|_| Ok(()), |id| killpg(id, None)),
    );
    assert!(
        poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(cleanup);
    let UnixCleanupState::Waiting { deadline } = fixture.owned.unix_cleanup else {
        panic!("real leader wait must remain pending");
    };
    assert!(fixture.owned.child.id().is_some());
    fixture.assert_member_alive().await;
    let mut reentered = Box::pin(fixture.owned.cleanup_unix_with(
        |_| panic!("wait reentry may not signal"),
        |id| killpg(id, None),
    ));
    assert!(
        poll_fn(|cx| Poll::Ready(reentered.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(reentered);
    assert!(
        matches!(fixture.owned.unix_cleanup, UnixCleanupState::Waiting { deadline: same } if same == deadline)
    );
    fixture.release_leader().await;
    fixture.request(b'Q').await;
    fixture
        .owned
        .cleanup_unix_with(
            |_| panic!("wait cancellation must not re-arm signal"),
            |id| killpg(id, None),
        )
        .await
        .unwrap();
    assert!(Instant::now() <= deadline);
    assert!(fixture.owned.child.id().is_none());
    assert!(fixture.owned.cleaned);
    fixture.close_root();
    drop(fixture);
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn first_signal_and_observation_failures_stay_typed_on_reentry_and_drop() {
    for signal_error in [Errno::EIO, Errno::EPERM] {
        let mut fixture = GroupFixture::start().await;
        let attempts = fixture.owned.cleanup_signal_attempts.clone();
        assert_eq!(
            fixture
                .owned
                .cleanup_unix_with(
                    |_| Err(std::io::Error::from_raw_os_error(signal_error as i32)),
                    |_| Err(Errno::EPERM)
                )
                .await,
            Err(LocalAgentError::CleanupFailed)
        );
        assert!(matches!(
            fixture.owned.unix_cleanup,
            UnixCleanupState::Failed
        ));
        assert!(!fixture.owned.cleaned);
        assert_eq!(
            fixture
                .owned
                .cleanup_unix_with(
                    |_| panic!("failed first action must not retry"),
                    |_| panic!("latched failure must not pretend to observe success")
                )
                .await,
            Err(LocalAgentError::CleanupFailed)
        );
        fixture.assert_member_alive().await;
        fixture.stop_member_and_close_root().await;
        drop(fixture);
        assert_eq!(attempts.load(Ordering::Relaxed), 1);
    }
    for query_error in [Errno::EPERM, Errno::EIO, Errno::EINVAL, Errno::ECHILD] {
        let mut fixture = GroupFixture::start().await;
        let attempts = fixture.owned.cleanup_signal_attempts.clone();
        assert_eq!(
            fixture
                .owned
                .cleanup_unix_with(|_| Ok(()), |_| Err(query_error))
                .await,
            Err(LocalAgentError::CleanupFailed)
        );
        assert!(!fixture.owned.cleaned);
        assert_eq!(
            fixture.owned.cleanup().await,
            Err(LocalAgentError::CleanupFailed)
        );
        fixture.assert_member_alive().await;
        fixture.stop_member_and_close_root().await;
        drop(fixture);
        assert_eq!(attempts.load(Ordering::Relaxed), 1);
    }
}

#[tokio::test]
async fn cancelled_observation_drop_cannot_act_on_present_numeric_identity() {
    let mut fixture = GroupFixture::start().await;
    let attempts = fixture.owned.cleanup_signal_attempts.clone();
    // Injected presence represents the only information numeric observation
    // gives after possible reuse; no actual reuse or unrelated group is made.
    let mut cleanup = Box::pin(fixture.owned.cleanup_unix_with(|_| Ok(()), |_| Ok(())));
    assert!(
        poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(cleanup);
    assert!(matches!(
        fixture.owned.unix_cleanup,
        UnixCleanupState::Observing { .. }
    ));
    assert!(!fixture.owned.cleaned);
    fixture.assert_member_alive().await;
    fixture.stop_member_and_close_root().await;
    drop(fixture);
    assert_eq!(
        attempts.load(Ordering::Relaxed),
        1,
        "actual OwnedChild Drop must not deliver a second numeric-group action"
    );
}

#[tokio::test]
async fn original_owned_cleanup_deadline_failure_does_not_reset_or_resignal() {
    let mut fixture = GroupFixture::start().await;
    let attempts = fixture.owned.cleanup_signal_attempts.clone();
    assert_eq!(
        fixture
            .owned
            .cleanup_unix_with(|_| Ok(()), |id| killpg(id, None))
            .await,
        Err(LocalAgentError::CleanupFailed)
    );
    assert!(matches!(
        fixture.owned.unix_cleanup,
        UnixCleanupState::Failed
    ));
    assert!(!fixture.owned.cleaned);
    fixture.assert_member_alive().await;
    assert_eq!(
        fixture
            .owned
            .cleanup_unix_with(
                |_| panic!("deadline reentry cannot signal"),
                |_| panic!("failed state cannot obtain a new deadline")
            )
            .await,
        Err(LocalAgentError::CleanupFailed)
    );
    fixture.stop_member_and_close_root().await;
    drop(fixture);
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn long_child_tmpdir_uses_short_private_root_for_member_and_held_leader() {
    let parent = tempfile::Builder::new()
        .prefix("ks-long-parent-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let long_tmpdir = parent.path().join("p".repeat(120));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&long_tmpdir)
        .unwrap();
    assert!(long_tmpdir.as_os_str().as_bytes().len() > 103);
    for held in [false, true] {
        let mut fixture = GroupFixture::start_with_tmpdir(held, Some(&long_tmpdir)).await;
        let private_root = fixture.path().to_owned();
        assert!(!private_root.starts_with(parent.path()));
        assert_eq!(
            std::fs::metadata(&private_root)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        for endpoint in ["member.sock", "leader.sock"] {
            let name = private_root.join(endpoint);
            assert!(name.as_os_str().as_bytes().len() <= 103);
            UnixAddr::new(&name).unwrap();
        }
        // Long-temp child roles assert their actual inherited TMPDIR before
        // binding. Their existing nonce/PID/group ACK proves real startup.
        fixture.assert_member_alive().await;
        fixture.owned.cleanup().await.unwrap();
        assert!(fixture.owned.child.id().is_none());
        assert_eq!(killpg(fixture.group.0, None), Err(Errno::ESRCH));
        fixture.close_root();
        assert!(!private_root.exists());
        assert!(long_tmpdir.exists(), "outer scratch is a separate owner");
    }
    let parent_path = parent.path().to_owned();
    parent.close().unwrap();
    assert!(!parent_path.exists());
}
