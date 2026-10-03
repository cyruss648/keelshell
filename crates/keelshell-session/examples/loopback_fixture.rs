//! Controlled GUI acceptance fixture. Never executes shell commands.
//! Run: cargo run -p keelshell-session --example loopback_fixture -- 300
//! Optional gateway: append a target fixture's loopback port. Only
//! `target.fixture.invalid:22` is then forwarded to `127.0.0.1:<target-port>`.
//! Credentials: fixture / keelshell-ui-test. Only 127.0.0.1 is bound.
//! Set `KEELSHELL_FIXTURE_IO_DELAY_MS` to 0–2000 to slow file reads/writes.
//! The fixed completion probe returns a fixture PATH without executing a shell.
//! Batch fixture commands return fixed bytes; KEELSHELL_FIXTURE_BATCH_EXIT is 0 or 7.

use russh::keys::{HashAlg, PrivateKey, ssh_key::Algorithm};
use russh::{Channel, ChannelId, ChannelMsg, Disconnect, server};
use std::{
    collections::HashMap,
    error::Error,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinSet,
};

#[path = "support/filesystem.rs"]
mod filesystem;

struct Fixture {
    root: Arc<PathBuf>,
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    jobs: JoinSet<()>,
    jump_target: Option<u16>,
    io_delay: Duration,
    batch_exit: u32,
}

impl server::Handler for Fixture {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "keelshell-ui-test" {
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
        reply.accept().await;
        Ok(())
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        let Some(destination) = jump_destination(self.jump_target, host, port) else {
            reply
                .reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        while self.jobs.try_join_next().is_some() {}
        if self.jobs.len() >= 64 {
            reply
                .reject(russh::ChannelOpenFailure::ResourceShortage)
                .await;
            return Ok(());
        }
        self.jobs.spawn(async move {
            let Ok(Ok(mut socket)) =
                tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(destination)).await
            else {
                reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
                return;
            };
            reply.accept().await;
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
        });
        Ok(())
    }
    async fn pty_request(
        &mut self,
        id: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(russh::Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        id: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(id)?;
        let Some(mut channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        self.jobs.spawn(async move {
            let banner = b"\r\n\x1b[36mKeelShell controlled loopback fixture\x1b[0m\r\nUTF-8 echo only; no commands are executed. Ctrl-D closes this shell.\r\nfixture> ";
            if channel.data(&banner[..]).await.is_err() { return; }
            while let Some(message) = channel.wait().await {
                match message {
                    ChannelMsg::Data { data } if data.contains(&4) => { let _ = channel.exit_status(0).await; break; },
                    ChannelMsg::Data { data } => {
                        let mut echo = Vec::with_capacity(data.len() + 32);
                        for byte in data {
                            if byte == b'\r' { echo.extend_from_slice(b"\r\nfixture> "); } else { echo.push(byte); }
                        }
                        if channel.data(&echo[..]).await.is_err() { break; }
                    },
                    ChannelMsg::Eof | ChannelMsg::Close => break,
                    _ => {},
                }
            }
            let _ = channel.close().await;
        });
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        // An exact protocol fixture, not a shell interpreter. Arbitrary input
        // remains refused, including other commands used by monitoring tools.
        if command == br#"command printf 'KEELSHELL_COMPLETION_V1\000%s\000' "$PATH""# {
            session.channel_success(id)?;
            session.data(id, b"KEELSHELL_COMPLETION_V1\0/bin\0".to_vec())?;
            session.exit_status_request(id, 0)?;
            session.eof(id)?;
            session.close(id)?;
            self.channels.remove(&id);
            return Ok(());
        }
        if matches!(
            command,
            b"keelshell-batch-fixture" | b"keelshell-batch-hold"
        ) {
            session.channel_success(id)?;
            session.data(
                id,
                "Batch fixture · 中文 stdout\n\u{1b}[31mraw control"
                    .as_bytes()
                    .to_vec(),
            )?;
            session.extended_data(id, 1, b"Batch fixture stderr\n".to_vec())?;
            if command == b"keelshell-batch-hold" {
                while self.jobs.try_join_next().is_some() {}
                if self.jobs.len() >= 64 {
                    session.close(id)?;
                    self.channels.remove(&id);
                    return Ok(());
                }
                let Some(mut channel) = self.channels.remove(&id) else {
                    return Err(russh::Error::Disconnect);
                };
                self.jobs.spawn(async move {
                    while let Some(message) = channel.wait().await {
                        // stdin EOF is expected immediately after exec admission.
                        if matches!(message, ChannelMsg::Close) {
                            break;
                        }
                    }
                    let _ = channel.close().await;
                });
                return Ok(());
            }
            session.exit_status_request(id, self.batch_exit)?;
            session.eof(id)?;
            session.close(id)?;
            self.channels.remove(&id);
            return Ok(());
        }
        session.channel_failure(id)?;
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
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        let Some(channel) = self.channels.remove(&id) else {
            return Err(russh::Error::Disconnect);
        };
        session.channel_success(id)?;
        let fs = filesystem::Filesystem::new(self.root.clone()).with_io_delay(self.io_delay);
        self.jobs.spawn(async move {
            russh_sftp::server::run(channel.into_stream(), fs).await;
        });
        Ok(())
    }
}

fn jump_destination(target: Option<u16>, host: &str, port: u32) -> Option<SocketAddr> {
    target
        .filter(|target| *target != 0 && host == "target.fixture.invalid" && port == 22)
        .map(|target| SocketAddr::from((Ipv4Addr::LOCALHOST, target)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let seconds = arguments
        .next()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(60);
    if !(1..=3600).contains(&seconds) {
        return Err("duration must be 1–3600 seconds".into());
    }
    let jump_target = arguments
        .next()
        .map(|value| value.parse::<u16>())
        .transpose()?;
    if jump_target == Some(0) || arguments.next().is_some() {
        return Err(
            "usage: loopback_fixture [1-3600 seconds] [nonzero target loopback port]".into(),
        );
    }
    let io_delay = match std::env::var("KEELSHELL_FIXTURE_IO_DELAY_MS") {
        Ok(value) => {
            let milliseconds = value.parse::<u64>()?;
            if milliseconds > 2000 {
                return Err("fixture IO delay must be 0–2000 milliseconds".into());
            }
            Duration::from_millis(milliseconds)
        }
        Err(std::env::VarError::NotPresent) => Duration::ZERO,
        Err(error) => return Err(error.into()),
    };
    let batch_exit = match std::env::var("KEELSHELL_FIXTURE_BATCH_EXIT") {
        Ok(value) if value == "0" => 0,
        Ok(value) if value == "7" => 7,
        Err(std::env::VarError::NotPresent) => 0,
        _ => return Err("fixture batch exit must be 0 or 7".into()),
    };
    let directory = tempfile::Builder::new()
        .prefix("keelshell-gui-fixture-")
        .tempdir()?;
    let root = Arc::new(directory.path().to_path_buf());
    std::fs::write(
        root.join("welcome.txt"),
        "KeelShell SFTP fixture\n中文内容\nAtomic save is supported on Unix.\n",
    )?;
    std::fs::create_dir(root.join("uploads"))?;
    std::fs::create_dir(root.join("bin"))?;
    std::fs::create_dir(root.join("中文 文档"))?;
    for name in [
        "报告 draft.txt",
        "report's draft.txt",
        "$(literal);name.txt",
    ] {
        std::fs::write(root.join(name), "Completion fixture; literal filename.\n")?;
    }
    for name in ["deploy-demo", "deploy-preview"] {
        let path = root.join("bin").join(name);
        // The fixture never executes these files. Permission bits exercise
        // remote PATH candidate discovery through ordinary SFTP attributes.
        std::fs::write(&path, "# fixture only\n")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    println!(
        "port={}\nfingerprint={}\nroot={}",
        listener.local_addr()?.port(),
        fingerprint,
        root.display()
    );
    let config = Arc::new(server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(100),
        auth_rejection_time_initial: Some(Duration::from_millis(100)),
        ..Default::default()
    });
    let mut clients = JoinSet::new();
    let (stop, _) = tokio::sync::watch::channel(false);
    let deadline = tokio::time::sleep(Duration::from_secs(seconds));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            _ = tokio::signal::ctrl_c() => break,
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                if clients.len() >= 16 { continue; }
                let config = config.clone();
                let root = root.clone();
                let mut stopped = stop.subscribe();
                clients.spawn(async move {
                    let fixture = Fixture { root, channels: HashMap::new(), jobs: JoinSet::new(), jump_target, io_delay, batch_exit };
                    if let Ok(mut running) = server::run_stream(config, stream, fixture).await {
                        let handle = running.handle();
                        tokio::select! {
                            _ = &mut running => {},
                            _ = stopped.changed() => {
                                let _ = handle.disconnect(Disconnect::ByApplication, "Fixture finished".into(), "en".into()).await;
                                let _ = tokio::time::timeout(Duration::from_secs(1), running).await;
                            },
                        }
                    }
                });
            },
            _ = clients.join_next(), if !clients.is_empty() => {},
        }
    }
    stop.send_replace(true);
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while clients.join_next().await.is_some() {}
    })
    .await;
    clients.abort_all();
    drop(listener);
    directory.close()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::jump_destination;

    #[test]
    fn optional_gateway_accepts_only_the_fixed_alias_and_maps_to_loopback() {
        assert_eq!(
            jump_destination(Some(2222), "target.fixture.invalid", 22),
            Some(([127, 0, 0, 1], 2222).into())
        );
        assert!(jump_destination(None, "target.fixture.invalid", 22).is_none());
        assert!(jump_destination(Some(0), "target.fixture.invalid", 22).is_none());
        for (host, port) in [
            ("127.0.0.1", 22),
            ("elsewhere.invalid", 22),
            ("target.fixture.invalid", 2222),
        ] {
            assert!(jump_destination(Some(2222), host, port).is_none());
        }
    }
}
