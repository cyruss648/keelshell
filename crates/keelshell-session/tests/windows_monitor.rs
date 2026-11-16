//! Controlled SSH frames test routing/authority; captures are a separate opt-in.
//! Neither fixture replay nor a Windows runner is native desktop acceptance.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use base64::Engine as _;
use keelshell_session::{
    SshAuth, SshOptions, SshSession,
    monitor::{
        CapabilityReason, CapabilityState, CpuSample, LinuxMonitor, MonitorError, MonitorPlatform,
        MonitorRuntime, ProcessInfo, SocketInfo,
    },
};
use russh::{
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use serde_json::json;
use tokio::sync::Semaphore;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, Debug)]
enum Operation {
    Posix = 0,
    Probe = 1,
    Resources = 2,
    Processes = 3,
    Sockets = 4,
}

struct Control {
    counts: [AtomicUsize; 5],
    commands: Mutex<Vec<String>>,
    unconfirmed: AtomicBool,
    missing_processes: AtomicBool,
    denied_resources: AtomicBool,
    opened: AtomicUsize,
    hold_open_at: AtomicUsize,
    open_started: Semaphore,
    open_release: Semaphore,
    hold_probe: AtomicBool,
    probe_started: Semaphore,
    probe_release: Semaphore,
    captures: Option<[Vec<u8>; 4]>,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            counts: std::array::from_fn(|_| AtomicUsize::new(0)),
            commands: Mutex::new(Vec::new()),
            unconfirmed: AtomicBool::new(false),
            missing_processes: AtomicBool::new(false),
            denied_resources: AtomicBool::new(false),
            opened: AtomicUsize::new(0),
            hold_open_at: AtomicUsize::new(0),
            open_started: Semaphore::new(0),
            open_release: Semaphore::new(0),
            hold_probe: AtomicBool::new(false),
            probe_started: Semaphore::new(0),
            probe_release: Semaphore::new(0),
            captures: None,
        }
    }
}

impl Control {
    fn count(&self, operation: Operation) -> usize {
        self.counts[operation as usize].load(Ordering::Acquire)
    }
}

fn frame(operation: Operation, failure: Option<&str>) -> Vec<u8> {
    let (name, data) = match operation {
        Operation::Probe => ("probe", json!({"os":"Win32NT"})),
        Operation::Resources => (
            "resources",
            json!({
                "boot_time":"1700000000","uptime_seconds":100.0,"cpu_percent":25.0,
                "memory_total_bytes":"1048576","memory_free_bytes":"262144",
                "network_count":1,"networks":[
                    {"name":"Ethernet 测试","received_bytes":"1234","transmitted_bytes":"5678"}
                ],
                "capacity_status":"ok","filesystem_count":1,"filesystems":[
                    {"source":"C:\\","mount":"C:\\","total_bytes":"1000",
                     "total_free_bytes":"400","available_bytes":"300"}
                ]
            }),
        ),
        Operation::Processes => (
            "processes",
            json!({"count":2,"processes":[
                {"pid":"0","name":"Idle","working_set_bytes":null},
                {"pid":"42","name":"测试 worker","working_set_bytes":"4096"}
            ]}),
        ),
        Operation::Sockets => (
            "sockets",
            json!({"count":2,"sockets":[
                {"protocol":"tcp","address":"127.0.0.1","port":"2222","pid":"42"},
                {"protocol":"udp","address":"::1","port":"5353","pid":"42"}
            ]}),
        ),
        Operation::Posix => return Vec::new(),
    };
    let mut encoded = serde_json::to_vec(&json!({
        "version":1,"operation":name,"platform":"Windows","runtime":"WindowsPowerShell",
        "runtime_version":"5.1.26100.123","units":"bytes seconds percent",
        "status":failure.unwrap_or("ok"),
        "data":if failure.is_some() {serde_json::Value::Null} else {data}
    }))
    .unwrap_or_else(|error| panic!("fixture JSON: {error}"));
    encoded.extend_from_slice(b"\r\n");
    encoded
}

fn operation(command: &str) -> TestResult<Operation> {
    if command.contains("@@KS:monitor-platform:1@@") {
        return Ok(Operation::Posix);
    }
    let encoded = command
        .strip_prefix("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ")
        .ok_or("unexpected non-production command prefix")?;
    if command.len() >= 8000 {
        return Err("command exceeds fixed default-shell budget".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
    if bytes.len() % 2 != 0 {
        return Err("incomplete UTF-16LE command".into());
    }
    let utf16: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .collect();
    let script = String::from_utf16(&utf16)?;
    if script.contains("Win32_OperatingSystem") {
        Ok(Operation::Resources)
    } else if script.contains("Get-NetTCPConnection") {
        Ok(Operation::Sockets)
    } else if script.contains("Get-Process") {
        Ok(Operation::Processes)
    } else {
        Ok(Operation::Probe)
    }
}

struct Peer(Arc<Control>);

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "test-only" {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _: russh::Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let ordinal = self.0.opened.fetch_add(1, Ordering::AcqRel) + 1;
        if self.0.hold_open_at.load(Ordering::Acquire) == ordinal {
            self.0.open_started.add_permits(1);
            tokio::time::timeout(Duration::from_secs(3), self.0.open_release.acquire())
                .await
                .map_err(|_| russh::Error::Disconnect)?
                .map_err(|_| russh::Error::Disconnect)?
                .forget();
        }
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        id: russh::ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let command = std::str::from_utf8(data).map_err(|_| russh::Error::Disconnect)?;
        let operation = operation(command).map_err(|_| russh::Error::Disconnect)?;
        self.0
            .commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(command.to_owned());
        self.0.counts[operation as usize].fetch_add(1, Ordering::Release);
        session.channel_success(id)?;
        if matches!(operation, Operation::Probe) && self.0.hold_probe.swap(false, Ordering::AcqRel)
        {
            self.0.probe_started.add_permits(1);
            self.0
                .probe_release
                .acquire()
                .await
                .map_err(|_| russh::Error::Disconnect)?
                .forget();
        }
        let missing = matches!(operation, Operation::Processes)
            && self.0.missing_processes.load(Ordering::Acquire);
        let denied = matches!(operation, Operation::Resources)
            && self.0.denied_resources.load(Ordering::Acquire);
        let failed_probe =
            matches!(operation, Operation::Probe) && self.0.unconfirmed.load(Ordering::Acquire);
        let body = if matches!(operation, Operation::Posix) {
            Vec::new()
        } else if failed_probe {
            b"untrusted shell diagnostic\r\n".to_vec()
        } else if let Some(captures) = &self.0.captures {
            captures[operation as usize - 1].clone()
        } else {
            frame(
                operation,
                if missing {
                    Some("command_missing")
                } else if denied {
                    Some("permission_denied")
                } else {
                    None
                },
            )
        };
        for chunk in body.chunks(4096) {
            session.data(id, chunk.to_vec())?;
        }
        let status = if matches!(operation, Operation::Posix) || missing || denied || failed_probe {
            1
        } else {
            0
        };
        session.exit_status_request(id, status)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
}

struct Fixture {
    session: SshSession,
    state: Arc<Control>,
    address: std::net::SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Fixture {
    async fn new(control: Control) -> TestResult<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x71; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let state = Arc::new(control);
        let peer = Peer(state.clone());
        let task = tokio::spawn(async move {
            if let Ok(Ok((socket, _))) =
                tokio::time::timeout(Duration::from_secs(5), listener.accept()).await
                && let Ok(connection) = server::run_stream(config, socket, peer).await
            {
                let _ = connection.await;
            }
        });
        let mut options = SshOptions::new(address.ip().to_string(), "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(zeroize::Zeroizing::new("test-only".into()));
        options.timeout = Duration::from_secs(3);
        let session = SshSession::connect(options).await?;
        Ok(Self {
            session,
            state,
            address,
            task,
        })
    }

    async fn close(mut self) -> TestResult {
        self.session.close().await?;
        if tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
            let _ = tokio::time::timeout(Duration::from_secs(2), &mut self.task).await?;
        }
        assert!(
            tokio::net::TcpStream::connect(self.address).await.is_err(),
            "owned listener survived"
        );
        Ok(())
    }
}

#[tokio::test]
async fn windows_socket_first_establishes_identity_and_all_read_only_fields_stay_honest()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control::default()).await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let sockets = monitor.listening_sockets().await?;
        assert_eq!(sockets.len(), 2);
        assert_eq!(sockets[0].state, "LISTEN");
        assert_eq!(sockets[1].state, "BOUND");
        let environment = monitor
            .capabilities()
            .await
            .environment
            .ok_or("missing Windows identity")?;
        assert_eq!(environment.platform, MonitorPlatform::Windows);
        assert_eq!(
            environment.runtime,
            MonitorRuntime::WindowsPowerShell {
                version: [5, 1, 26100, 123]
            }
        );
        let sample = monitor.snapshot().await?;
        assert_eq!(sample.cpu, CpuSample::ReportedPercent(25.));
        assert_eq!(sample.load_average, None);
        assert_eq!(sample.memory.free_bytes, Some(262144));
        assert_eq!(sample.memory.used_bytes(), None);
        assert_eq!(sample.cpu.cumulative(), None);
        assert!(sample.rates_since(&sample).networks.is_empty());
        assert_eq!(sample.filesystems[0].used_bytes, 600);
        assert_eq!(sample.filesystems[0].available_bytes, 300);
        let processes = monitor.processes().await?;
        assert_eq!(processes.iter().map(|p| p.pid).collect::<Vec<_>>(), [0, 42]);
        assert_eq!(processes[1].working_set_bytes, Some(4096));
        assert!(
            processes
                .iter()
                .all(|p| p.user.is_none() && p.cpu_percent.is_none() && p.memory_percent.is_none())
        );
        assert_eq!(fixture.state.count(Operation::Posix), 1);
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_clones_share_independent_negative_cache_and_explicit_reset_redetects() -> TestResult
{
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control {
            missing_processes: AtomicBool::new(true),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let clone = monitor.clone();
        assert!(matches!(
            monitor.processes().await,
            Err(MonitorError::CapabilityUnavailable {
                reason: CapabilityReason::CommandMissing,
                ..
            })
        ));
        let before = monitor.capabilities().await;
        fixture
            .state
            .missing_processes
            .store(false, Ordering::Release);
        assert!(clone.processes().await.is_err());
        assert_eq!(fixture.state.count(Operation::Processes), 1);
        assert!(
            clone.snapshot().await.is_ok(),
            "process failure must not suppress resources"
        );
        assert_eq!(monitor.capabilities().await.processes, before.processes);
        monitor.invalidate_pending_capabilities();
        assert!(monitor.snapshot().await.is_ok());
        assert_eq!(
            fixture.state.count(Operation::Probe),
            1,
            "pause preserves completed identity"
        );
        clone.reset_capabilities().await;
        assert_eq!(monitor.capabilities().await.environment, None);
        assert_eq!(clone.processes().await?.len(), 2);
        assert_eq!(fixture.state.count(Operation::Probe), 2);
        assert_eq!(fixture.state.count(Operation::Processes), 2);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_resource_permission_cache_preserves_age_until_explicit_recheck() -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control {
            denied_resources: AtomicBool::new(true),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::CapabilityUnavailable {
                reason: CapabilityReason::PermissionDenied,
                ..
            })
        ));
        let before = monitor.capabilities().await;
        let report = monitor.capability_report().await;
        fixture
            .state
            .denied_resources
            .store(false, Ordering::Release);
        for _ in 0..2 {
            assert!(matches!(
                monitor.clone().snapshot().await,
                Err(MonitorError::CapabilityUnavailable {
                    reason: CapabilityReason::PermissionDenied,
                    ..
                })
            ));
        }
        assert_eq!(monitor.capabilities().await, before);
        assert!(monitor.accepts_capability_report(&report));
        assert_eq!(fixture.state.count(Operation::Resources), 1);
        assert_eq!(monitor.processes().await?.len(), 2);
        monitor.reset_capabilities().await;
        assert!(!monitor.accepts_capability_report(&report));
        assert!(monitor.snapshot().await.is_ok());
        assert_eq!(fixture.state.count(Operation::Probe), 2);
        assert_eq!(fixture.state.count(Operation::Resources), 2);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_pending_identity_cannot_start_a_collector_after_reset() -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control {
            hold_probe: AtomicBool::new(true),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let pending_monitor = monitor.clone();
        let pending = tokio::spawn(async move { pending_monitor.snapshot().await });
        tokio::time::timeout(
            Duration::from_secs(3),
            fixture.state.probe_started.acquire(),
        )
        .await??
        .forget();
        monitor.reset_capabilities().await;
        fixture.state.probe_release.add_permits(1);
        assert!(matches!(pending.await?, Err(MonitorError::Cancelled)));
        assert_eq!(fixture.state.count(Operation::Resources), 0);
        assert_eq!(monitor.capabilities().await.environment, None);
        assert_eq!(
            monitor.capabilities().await.resources.state,
            CapabilityState::Unknown
        );
        assert_eq!(monitor.processes().await?.len(), 2);
        assert_eq!(fixture.state.count(Operation::Probe), 2);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn revoked_identity_channel_admission_never_dispatches_posix_or_windows_probe() -> TestResult
{
    tokio::time::timeout(Duration::from_secs(20), async {
        for ordinal in [1, 2] {
            for reset in [false, true] {
                let fixture = Fixture::new(Control {
                    hold_open_at: AtomicUsize::new(ordinal),
                    ..Control::default()
                })
                .await?;
                let monitor = LinuxMonitor::new(fixture.session.clone());
                let reader = monitor.clone();
                let pending = tokio::spawn(async move { reader.snapshot().await });
                fixture.state.open_started.acquire().await?.forget();
                if reset {
                    monitor.reset_capabilities().await;
                } else {
                    monitor.invalidate_pending_capabilities();
                }
                fixture.state.open_release.add_permits(1);
                assert!(matches!(pending.await?, Err(MonitorError::Cancelled)));
                assert_eq!(fixture.state.count(Operation::Posix), ordinal - 1);
                assert_eq!(fixture.state.count(Operation::Probe), 0);
                assert_eq!(fixture.state.count(Operation::Resources), 0);
                assert_eq!(monitor.capabilities().await.environment, None);
                assert_eq!(
                    monitor.capabilities().await.resources.state,
                    CapabilityState::Unknown
                );
                fixture.close().await?;
            }
        }
        TestResult::Ok(())
    })
    .await?
}

#[tokio::test]
async fn revoked_cached_collector_channel_admission_never_dispatches_any_read_operation()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(25), async {
        for operation in [
            Operation::Resources,
            Operation::Processes,
            Operation::Sockets,
        ] {
            for reset in [false, true] {
                let fixture = Fixture::new(Control::default()).await?;
                let monitor = LinuxMonitor::new(fixture.session.clone());
                monitor.snapshot().await?;
                let before = monitor.capabilities().await;
                let commands = fixture
                    .state
                    .commands
                    .lock()
                    .map_err(|_| "commands lock")?
                    .len();
                let report = monitor.capability_report().await;
                fixture.state.hold_open_at.store(
                    fixture.state.opened.load(Ordering::Acquire) + 1,
                    Ordering::Release,
                );
                let reader = monitor.clone();
                let pending = tokio::spawn(async move {
                    match operation {
                        Operation::Resources => reader.snapshot().await.map(|_| ()),
                        Operation::Processes => reader.processes().await.map(|_| ()),
                        Operation::Sockets => reader.listening_sockets().await.map(|_| ()),
                        _ => unreachable!("only read collectors enter this fixture"),
                    }
                });
                fixture.state.open_started.acquire().await?.forget();
                if reset {
                    monitor.reset_capabilities().await;
                } else {
                    monitor.invalidate_pending_capabilities();
                }
                fixture.state.open_release.add_permits(1);
                assert!(matches!(pending.await?, Err(MonitorError::Cancelled)));
                assert_eq!(
                    fixture
                        .state
                        .commands
                        .lock()
                        .map_err(|_| "commands lock")?
                        .len(),
                    commands,
                    "{operation:?} must not send exec after revoked admission"
                );
                assert!(!monitor.accepts_capability_report(&report));
                assert_eq!(
                    monitor.capabilities().await,
                    if reset { Default::default() } else { before }
                );
                fixture.close().await?;
            }
        }
        TestResult::Ok(())
    })
    .await?
}

#[tokio::test]
async fn simultaneous_readers_deduplicate_identity_probes_on_one_connection() -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control::default()).await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let clone = monitor.clone();
        let (resources, processes, sockets) = tokio::join!(
            monitor.snapshot(),
            clone.processes(),
            monitor.listening_sockets(),
        );
        assert!(resources.is_ok() && processes.is_ok() && sockets.is_ok());
        assert_eq!(fixture.state.count(Operation::Posix), 1);
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn two_failed_probes_leave_environment_unknown_and_do_not_cache_a_missing_command()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control {
            unconfirmed: AtomicBool::new(true),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::EnvironmentUnconfirmed)
        ));
        let observed = monitor.capabilities().await;
        assert_eq!(observed.environment, None);
        assert_eq!(observed.resources.state, CapabilityState::Unknown);
        assert!(matches!(
            monitor.processes().await,
            Err(MonitorError::EnvironmentUnconfirmed)
        ));
        assert_eq!(fixture.state.count(Operation::Probe), 2);
        fixture.state.unconfirmed.store(false, Ordering::Release);
        assert!(
            monitor.snapshot().await.is_ok(),
            "unknown identity may be re-observed"
        );
        assert_eq!(fixture.state.count(Operation::Probe), 3);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
async fn windows_reviewed_linux_actions_fail_before_reading_or_signalling_processes() -> TestResult
{
    tokio::time::timeout(Duration::from_secs(15), async {
        let fixture = Fixture::new(Control::default()).await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let socket = SocketInfo {
            protocol: "tcp".into(),
            state: "LISTEN".into(),
            local: "127.0.0.1:2222".into(),
            peer: "*".into(),
            process: Some("PID 42".into()),
        };
        assert!(matches!(
            monitor.probe_tcp(&socket).await,
            Err(MonitorError::Unsupported(_))
        ));
        let process = ProcessInfo {
            pid: 42,
            user: None,
            cpu_percent: None,
            memory_percent: None,
            working_set_bytes: Some(4096),
            command: "test".into(),
        };
        assert!(matches!(
            monitor.inspect_process(&process).await,
            Err(MonitorError::Unsupported(_))
        ));
        assert_eq!(fixture.state.count(Operation::Processes), 0);
        assert_eq!(fixture.state.count(Operation::Sockets), 0);
        assert_eq!(fixture.state.count(Operation::Resources), 0);
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}

#[tokio::test]
#[ignore = "requires genuine Windows API frames from the isolated Windows monitor lab"]
async fn captured_windows_api_frames_are_accepted_through_authenticated_monitor_routing()
-> TestResult {
    tokio::time::timeout(Duration::from_secs(20), async {
        let directory = PathBuf::from(
            std::env::var_os("KEELSHELL_WINDOWS_MONITOR_CAPTURE_DIR")
                .ok_or("capture directory required")?,
        );
        let mut captures: [Vec<u8>; 4] = std::array::from_fn(|_| Vec::new());
        for (slot, filename) in captures.iter_mut().zip([
            "probe.stdout",
            "resources.stdout",
            "processes.stdout",
            "sockets.stdout",
        ]) {
            *slot = std::fs::read(directory.join(filename))?;
            if slot.len() > 2 * 1024 * 1024 {
                return Err("capture exceeds the production output bound".into());
            }
        }
        let fixture = Fixture::new(Control {
            captures: Some(captures),
            ..Control::default()
        })
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let sample = monitor.snapshot().await?;
        assert_eq!(sample.platform, MonitorPlatform::Windows);
        assert!(sample.memory.total_bytes > 0);
        assert!(sample.memory.available_bytes.is_none() && sample.memory.used_bytes().is_none());
        assert_eq!(sample.load_average, None);
        assert!(sample.cpu.cumulative().is_none());
        let _processes = monitor.processes().await?;
        let _sockets = monitor.listening_sockets().await?;
        assert_eq!(fixture.state.count(Operation::Probe), 1);
        fixture.close().await
    })
    .await?
}
