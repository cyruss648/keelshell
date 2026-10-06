//! Independent interval and owned TCP cases; generated proc text is controlled data.
use keelshell_session::{
    SessionError, SshAuth, SshOptions, SshSession,
    monitor::{
        DiskIoError, DiskIoSnapshot, DiskRateUnavailable, LinuxMonitor, MonitorError, Snapshot,
    },
};
use russh::{
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const BOOT: &str = "297a70af-36d7-4e83-a4ca-07bf3bf7fa8e";

fn row(name: &str, minor: u32, fields: &[u64]) -> String {
    format!(
        "259 {minor} {name} {}\n",
        fields
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn resources(uptime: u64, increment: u64, optional: &str) -> String {
    format!(
        "@@KS:platform@@\nLinux\n@@KS:stat@@\ncpu {} 0 0 {} 0 0 0 0\nbtime 1700000000\n@@KS:meminfo@@\nMemTotal: 2048 kB\nMemAvailable: 1024 kB\n@@KS:loadavg@@\n0.1 0.2 0.3\n@@KS:uptime@@\n{uptime} 10\n@@KS:net@@\neth0: {} 0 0 0 0 0 0 0 {} 0 0 0 0 0 0 0\n@@KS:df@@\nFilesystem 1024-blocks Used Available Capacity Mounted on\n/dev/root 1000 400 600 40% /\n{optional}@@KS:end@@\n",
        100 + increment,
        300 + increment,
        1000 + increment,
        2000 + increment,
    )
}

fn disk_section(fields: &[u64]) -> String {
    format!(
        "@@KS:bootid@@\n{BOOT}\n@@KS:diskstats@@\n{}",
        row("nvme0n1", 0, fields)
    )
}

#[test]
fn invalid_optional_sample_breaks_disk_baseline_and_preserves_other_rates()
-> Result<(), Box<dyn std::error::Error>> {
    let before = Snapshot::parse(&resources(100, 0, &disk_section(&[10; 17])))?;
    for optional in [
        "".to_owned(),
        format!(
            "@@KS:bootid@@\n\n@@KS:diskstats@@\n{}",
            row("nvme0n1", 0, &[20; 17])
        ),
        format!("@@KS:bootid@@\n{BOOT}\n@@KS:diskstats@@\n259 0 nvme0n1 1 2 3 4\n"),
        format!("@@KS:bootid@@\n{BOOT}\n@@KS:diskstats@@\n!unavailable\n"),
    ] {
        let bad = Snapshot::parse(&resources(102, 2, &optional))?;
        assert!(bad.disk_io.is_err());
        let other = bad.rates_since(&before);
        assert_eq!(other.cpu_busy_percent, Some(50.));
        assert_eq!(other.networks[0].received_bytes_per_second, Some(1.));
        assert!(other.disks.is_empty());
        let recovery = Snapshot::parse(&resources(104, 4, &disk_section(&[30; 17])))?;
        assert_eq!(
            recovery.rates_since(&bad).disks[0].observation,
            Err(DiskRateUnavailable::FirstSample)
        );
        let next = Snapshot::parse(&resources(106, 6, &disk_section(&[40; 17])))?;
        assert_eq!(
            next.rates_since(&recovery).disks[0]
                .observation
                .as_ref()
                .map(|value| value.read_bytes_per_second),
            Ok(2560.)
        );
    }
    Ok(())
}

#[test]
fn reset_is_per_device_and_falling_inflight_does_not_mask_completed_io()
-> Result<(), Box<dyn std::error::Error>> {
    let before = DiskIoSnapshot::parse(
        BOOT,
        &(row("nvme0n1", 0, &[100; 17]) + &row("nvme0n1p1", 1, &[100; 17])),
    )?;
    let mut whole = [120; 17];
    whole[8] = 0;
    let mut partition = [120; 17];
    partition[12] = 99; // A discard merge reset invalidates this row only.
    let after = DiskIoSnapshot::parse(
        BOOT,
        &(row("nvme0n1p1", 1, &partition) + &row("nvme0n1", 0, &whole)),
    )?;
    let rates = after.rates_since(Some(&before), 4., true);
    assert_eq!(rates.len(), 2);
    assert_eq!(rates[0].observation, Err(DiskRateUnavailable::CounterReset));
    let valid = rates[1]
        .observation
        .as_ref()
        .map_err(|_| "whole row unavailable")?;
    assert_eq!(valid.read_bytes_per_second, 2560.);
    assert_eq!(valid.reads_per_second, 5.);
    assert_eq!(after.devices[1].in_flight(), 0);
    let absent = DiskIoSnapshot::parse(BOOT, &row("nvme0n1", 0, &whole))?;
    let returning = DiskIoSnapshot::parse(BOOT, &row("nvme0n1p1", 1, &[130; 17]))?;
    assert_eq!(
        returning.rates_since(Some(&absent), 4., true)[0].observation,
        Err(DiskRateUnavailable::NewDevice)
    );
    Ok(())
}

#[test]
fn complete_parallel_request_times_are_not_capped_to_wall_time_or_activity()
-> Result<(), Box<dyn std::error::Error>> {
    let before = DiskIoSnapshot::parse(BOOT, &row("nvme0n1", 0, &[0; 11]))?;
    let mut fields = [0; 11];
    fields[0] = 60;
    fields[2] = 480;
    fields[3] = 1800;
    fields[8] = 7;
    fields[9] = 125;
    fields[10] = 4000;
    let after = DiskIoSnapshot::parse(BOOT, &row("nvme0n1", 0, &fields))?;
    let rates = after.rates_since(Some(&before), 1., true);
    let valid = rates[0]
        .observation
        .as_ref()
        .map_err(|_| "parallel observation unavailable")?;
    assert_eq!(valid.read_milliseconds_per_request, Some(30.));
    assert_eq!(valid.write_milliseconds_per_request, None);
    assert_eq!(valid.read_bytes_per_second, 245_760.);
    assert_eq!(valid.activity_milliseconds, 125);
    assert_eq!(after.devices[0].in_flight(), 7);
    assert_eq!(
        DiskIoSnapshot::parse("", &row("nvme0n1", 0, &[0; 11])),
        Err(DiskIoError::BootIdentity)
    );
    Ok(())
}

struct SlowPeer {
    commands: Arc<Mutex<Vec<Vec<u8>>>>,
}
impl server::Handler for SlowPeer {
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
        session.channel_success(id)?;
        for _ in 0..10 {
            session.data(id, b"partial".to_vec())?;
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
        session.exit_status_request(id, 0)?;
        session.eof(id)?;
        session.close(id)?;
        Ok(())
    }
}

#[tokio::test]
async fn optional_collector_does_not_renew_exec_deadline_on_partial_data()
-> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x39; 32]));
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let commands = Arc::new(Mutex::new(Vec::new()));
    let peer = SlowPeer {
        commands: commands.clone(),
    };
    let mut task = tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await
            && let Ok(connection) = server::run_stream(config, stream, peer).await
        {
            let _ = connection.await;
        }
    });
    let mut options = SshOptions::new(address.ip().to_string(), "fixture");
    options.port = address.port();
    options.expected_host_key = Some(fingerprint);
    options.auth = SshAuth::Password(zeroize::Zeroizing::new("test-only".into()));
    options.timeout = Duration::from_millis(250);
    let connection = SshSession::connect(options).await?;
    let start = Instant::now();
    let outcome = LinuxMonitor::new(connection.clone()).snapshot().await;
    let elapsed = start.elapsed();
    let close = connection.close().await;
    task.abort();
    let joined = tokio::time::timeout(Duration::from_secs(2), &mut task).await;
    assert!(joined.is_ok(), "owned server task did not settle");
    assert!(
        tokio::net::TcpStream::connect(address).await.is_err(),
        "owned listener survived"
    );
    assert!(matches!(
        outcome,
        Err(MonitorError::Session(SessionError::Timeout("SSH exec")))
    ));
    assert!(
        elapsed >= Duration::from_millis(200) && elapsed < Duration::from_millis(700),
        "{elapsed:?}"
    );
    close?;
    let commands = commands.lock().map_err(|_| "poisoned command capture")?;
    assert_eq!(commands.len(), 1);
    let expected = include_str!("../src/monitor.rs")
        .split_once("const SNAPSHOT_SCRIPT: &str = r#\"")
        .ok_or("collector start")?
        .1
        .split_once("\"#;")
        .ok_or("collector end")?
        .0;
    assert_eq!(commands[0], expected.as_bytes());
    Ok(())
}
