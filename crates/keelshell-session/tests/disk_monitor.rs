//! Owned TCP/SSH protocol fixtures. Proc text is controlled, not a real Linux claim.
use keelshell_session::{
    SessionError, SshAuth, SshOptions, SshSession,
    monitor::{DiskIoError, DiskRateUnavailable, LinuxMonitor, MonitorError},
};
use russh::{
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

const BOOT: &str = "11ac8d57-72c6-4ee6-93bf-68724d27e715";
fn body(uptime: u64, boot: &str, rows: &str) -> Vec<u8> {
    format!("@@KS:platform@@\nLinux\n@@KS:stat@@\ncpu 100 20 30 400 50 0 0 0\nbtime 1700000000\n@@KS:meminfo@@\nMemTotal: 1024 kB\nMemAvailable: 256 kB\n@@KS:loadavg@@\n1.25 0.50 0.25 1/100 45\n@@KS:uptime@@\n{uptime}.00 75.00\n@@KS:net@@\nlo: 100 1 0 0 0 0 0 0 200 1 0 0 0 0 0 0\n@@KS:df@@\nFilesystem 1024-blocks Used Available Capacity Mounted on\n/dev/root 1000 600 350 64% /\n@@KS:bootid@@\n{boot}\n@@KS:diskstats@@\n{rows}\n@@KS:end@@\n").into_bytes()
}
type Commands = Arc<Mutex<Vec<Vec<u8>>>>;
struct Peer {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    commands: Commands,
}
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
        reply.accept().await;
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: russh::ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.commands
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(data.to_vec());
        let response = self
            .responses
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .pop_front()
            .ok_or(russh::Error::Disconnect)?;
        session.channel_success(id)?;
        for chunk in response.chunks(4096) {
            session.data(id, chunk.to_vec())?;
        }
        session.exit_status_request(id, 0)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
}
struct Fixture {
    session: SshSession,
    commands: Commands,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn new(responses: Vec<Vec<u8>>) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x49; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let commands = Arc::new(Mutex::new(Vec::new()));
        let peer = Peer {
            responses: Arc::new(Mutex::new(responses.into())),
            commands: commands.clone(),
        };
        let task = tokio::spawn(async move {
            if let Ok(Ok((socket, _))) =
                tokio::time::timeout(Duration::from_secs(5), listener.accept()).await
                && let Ok(session) = server::run_stream(config, socket, peer).await
            {
                let _ = session.await;
            }
        });
        let mut options = SshOptions::new(address.ip().to_string(), "fixture");
        options.port = address.port();
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(zeroize::Zeroizing::new("test-only".into()));
        options.timeout = Duration::from_secs(5);
        let session = SshSession::connect(options).await?;
        Ok(Self {
            session,
            commands,
            task,
        })
    }
}

#[tokio::test]
async fn authenticated_snapshot_reads_exact_optional_counters_without_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    tokio::time::timeout(Duration::from_secs(12), async {
        let before = "8 0 sda 10 0 100 20 5 0 50 10 3 15 18\n8 1 sda1 2 0 10 4 1 0 5 2 0 3 4\n";
        let after = "8 0 sda 14 0 120 28 9 0 90 18 1 40 50\n8 1 sda1 4 0 15 8 3 0 10 6 0 5 7\n";
        let fixture = Fixture::new(vec![
            body(100, BOOT, before),
            body(102, BOOT, after),
            body(104, BOOT, "8 1 sda1 6 0 20 12 5 0 15 10 0 7 9\n"),
            body(106, "a1ac8d57-72c6-4ee6-93bf-68724d27e715", after),
            body(108, "", after),
        ])
        .await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        let first = monitor.snapshot().await?;
        let second = monitor.snapshot().await?;
        let rates = second.rates_since(&first);
        assert_eq!(rates.disks.len(), 2);
        let rate = rates.disks[0]
            .observation
            .as_ref()
            .map_err(|_| "expected exact disk rate")?;
        assert_eq!(rate.read_bytes_per_second, 5120.);
        assert_eq!(rate.written_bytes_per_second, 10240.);
        assert_eq!(rate.reads_per_second, 2.);
        assert_eq!(rate.read_milliseconds_per_request, Some(2.));
        let third = monitor.snapshot().await?;
        let rates = third.rates_since(&second);
        assert!(rates.disks.iter().any(|row| row.device.name == "sda"
            && row.observation == Err(DiskRateUnavailable::Disappeared)));
        let reboot = monitor.snapshot().await?;
        assert!(
            reboot
                .rates_since(&third)
                .disks
                .iter()
                .filter(|row| row.device.name == "sda1")
                .all(|row| row.observation == Err(DiskRateUnavailable::BootChanged))
        );
        let optional = monitor.snapshot().await?;
        assert_eq!(optional.disk_io, Err(DiskIoError::BootIdentity));
        assert_eq!(optional.memory.total_bytes, 1048576);
        let commands = fixture
            .commands
            .lock()
            .map_err(|_| "fixture commands poisoned")?
            .clone();
        assert_eq!(commands.len(), 5);
        assert!(commands.windows(2).all(|pair| pair[0] == pair[1]));
        let command = std::str::from_utf8(&commands[0])?;
        assert!(
            command.contains("cat /proc/diskstats")
                && command.contains("cat /proc/sys/kernel/random/boot_id")
        );
        assert!(
            !command.contains("kill") && !command.contains("sudo") && !command.contains("install")
        );
        fixture.session.close().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await?
}

#[tokio::test]
async fn malformed_utf8_and_bounded_response_are_typed_failures()
-> Result<(), Box<dyn std::error::Error>> {
    tokio::time::timeout(Duration::from_secs(12), async {
        let fixture = Fixture::new(vec![vec![0xff], vec![b'x'; 2 * 1024 * 1024 + 1]]).await?;
        let monitor = LinuxMonitor::new(fixture.session.clone());
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::InvalidData("non-UTF-8 command output"))
        ));
        assert!(matches!(
            monitor.snapshot().await,
            Err(MonitorError::Session(SessionError::OutputLimit(2_097_152)))
        ));
        assert_eq!(
            fixture
                .commands
                .lock()
                .map_err(|_| "fixture commands poisoned")?
                .len(),
            2
        );
        fixture.session.close().await?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await?
}

/// Opt-in parsing of two independently captured, genuine Linux collector outputs.
/// Transport fixtures above remain separate; these files never imply native GUI
/// acceptance or a physical disk outside the sampler's visible kernel namespace.
#[test]
#[ignore = "requires two private genuine Linux collector samples"]
fn genuine_linux_capture_has_boot_bound_per_device_intervals()
-> Result<(), Box<dyn std::error::Error>> {
    use keelshell_session::monitor::Snapshot;
    fn sample(name: &str) -> Result<Snapshot, Box<dyn std::error::Error>> {
        let path = std::env::var_os(name).ok_or("private sample path is required")?;
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() > 2 * 1024 * 1024 {
            return Err("private sample exceeds collector output limit".into());
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        file.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("private sample changed beyond collector limit".into());
        }
        Ok(Snapshot::parse(std::str::from_utf8(&bytes)?)?)
    }
    let first = sample("KEELSHELL_DISK_SAMPLE_FIRST")?;
    let second = sample("KEELSHELL_DISK_SAMPLE_SECOND")?;
    assert!(second.uptime_seconds > first.uptime_seconds);
    assert_eq!(first.boot_time, second.boot_time);
    assert!(
        !first
            .disk_io
            .as_ref()
            .map_err(Clone::clone)?
            .devices
            .is_empty()
    );
    assert!(
        !second
            .disk_io
            .as_ref()
            .map_err(Clone::clone)?
            .devices
            .is_empty()
    );
    let rates = second.rates_since(&first);
    assert!(!rates.disks.is_empty());
    assert!(rates.disks.iter().all(|row| match &row.observation {
        Ok(rate) =>
            rate.read_bytes_per_second.is_finite()
                && rate.written_bytes_per_second.is_finite()
                && rate.interval_seconds > 0.,
        Err(reason) => matches!(
            reason,
            DiskRateUnavailable::NewDevice
                | DiskRateUnavailable::Disappeared
                | DiskRateUnavailable::CounterReset
                | DiskRateUnavailable::LayoutChanged
                | DiskRateUnavailable::Overflow
        ),
    }));
    assert!(rates.disks.iter().any(|row| row.observation.is_ok()));
    Ok(())
}
