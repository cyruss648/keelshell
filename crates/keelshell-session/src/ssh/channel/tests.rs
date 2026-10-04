//! Real backpressure and ownership-transfer cancellation, without timing hooks.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::{Arc, Mutex},
    time::Duration,
};

use russh::{
    Channel, ChannelId, ChannelMsg,
    keys::{HashAlg, PrivateKey, ssh_key::Algorithm},
    server,
};
use tokio::{net::TcpListener, task::JoinHandle};
use zeroize::Zeroizing;

use super::{PendingChannel, open_session};
use crate::{SshAuth, SshOptions, SshSession};

struct Handler {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    closed: Arc<Mutex<HashSet<ChannelId>>>,
}
impl server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(&mut self, _: &str, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
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
    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Ok(mut closed) = self.closed.lock() {
            closed.insert(id);
        }
        self.channels.remove(&id);
        Ok(())
    }
    async fn exec_request(
        &mut self,
        id: ChannelId,
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if command == b"fill client receiver" {
            // Each frame consumes one slot; the client Channel is deliberately
            // unread so its 100-slot receive queue blocks the protocol loop.
            for _ in 0..160 {
                session.data(id, b"x".to_vec())?;
            }
        } else {
            session.channel_success(id)?;
            session.data(id, command.to_vec())?;
            session.exit_status_request(id, 0)?;
            session.eof(id)?;
            session.close(id)?;
        }
        Ok(())
    }
}

struct Fixture {
    session: SshSession,
    closed: Arc<Mutex<HashSet<ChannelId>>>,
    task: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn new() -> Result<Self, Box<dyn Error>> {
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            auth_rejection_time: Duration::from_millis(1),
            auth_rejection_time_initial: Some(Duration::from_millis(1)),
            ..Default::default()
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let closed = Arc::new(Mutex::new(HashSet::new()));
        let observed = closed.clone();
        let task = tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await
                && let Ok(running) = server::run_stream(
                    config,
                    socket,
                    Handler {
                        channels: HashMap::new(),
                        closed: observed,
                    },
                )
                .await
            {
                let _ = running.await;
            }
        });
        let session = SshSession::connect(SshOptions {
            host: address.ip().to_string(),
            port: address.port(),
            username: "fixture".into(),
            proxy: None,
            expected_host_key: Some(fingerprint),
            auth: SshAuth::Password(Zeroizing::new("fixture".into())),
            timeout: Duration::from_secs(5),
        })
        .await?;
        Ok(Self {
            session,
            closed,
            task,
        })
    }
    async fn wait_closed(&self, id: ChannelId) -> Result<(), Box<dyn Error>> {
        tokio::time::timeout(Duration::from_secs(3), async {
            while !self
                .closed
                .lock()
                .unwrap_or_else(|_| panic!("closed mutex"))
                .contains(&id)
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await?;
        Ok(())
    }
}

fn channel_id(channel: &PendingChannel) -> ChannelId {
    channel
        .channel
        .as_ref()
        .unwrap_or_else(|| panic!("owned channel"))
        .id()
}

#[tokio::test]
async fn cancelling_explicit_close_while_protocol_sender_is_full_keeps_cleanup_owner()
-> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new().await?;
    let victim = open_session(&fixture.session).await?;
    let victim_id = channel_id(&victim);
    let mut flood = open_session(&fixture.session).await?;
    flood
        .channel
        .as_ref()
        .ok_or("flood channel")?
        .exec(false, "fill client receiver")
        .await?;

    // Synchronize on a packet that has actually reached the client's channel
    // receiver before filling the protocol command queue. Merely awaiting the
    // exec request is insufficient: the server can still be scheduling the
    // 160 DATA packets while the keepalives are admitted.
    {
        let channel = flood.channel.as_mut().ok_or("flood channel")?;
        let first = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Data { data }) => break Ok(data),
                    Some(_) => {}
                    None => break Err("flood channel closed before first data"),
                }
            }
        })
        .await??;
        assert_eq!(first.as_ref(), b"x", "unexpected first flood packet");
    }

    // Fill russh's ten-slot command queue only after the protocol has observed
    // flood data. The final timed-out send proves the actual client protocol
    // sender is backpressured rather than merely racing the fixture startup.
    let mut blocked = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(
            Duration::from_millis(30),
            fixture.session.handle.send_keepalive(false),
        )
        .await
        {
            Ok(result) => {
                result?;
                // A ready bounded send does not necessarily yield to the
                // protocol task. Yield explicitly so incoming DATA can fill
                // the channel receiver between command sends.
                tokio::task::yield_now().await;
            }
            Err(_) => {
                blocked = true;
                break;
            }
        }
    }
    assert!(
        blocked,
        "fixture must block the actual client protocol sender"
    );
    let mut closing = Box::pin(victim.close());
    assert!(
        tokio::time::timeout(Duration::from_millis(25), &mut closing)
            .await
            .is_err(),
        "CLOSE enqueue must be suspended"
    );
    // Dropping the explicitly cancelled future invokes PendingChannel's
    // independent Drop owner, which retries the close after the queue drains.
    drop(closing);
    // Releasing the real receive queue allows the independent Drop owner to
    // enqueue CLOSE; cancelling the caller must not abandon its channel.
    tokio::time::timeout(Duration::from_secs(2), async {
        let channel = flood.channel.as_mut().ok_or("flood channel")?;
        // The first DATA packet was consumed above. Count it explicitly so the
        // drain proves that the fixture delivered every one of its 160 packets.
        let mut packets = 1;
        while packets < 160 {
            match channel.wait().await {
                Some(ChannelMsg::Data { data }) => {
                    assert_eq!(data.as_ref(), b"x", "unexpected flood packet");
                    packets += 1;
                }
                Some(_) => {}
                None => return Err("flood channel closed early"),
            }
        }
        Ok::<_, &'static str>(())
    })
    .await??;
    flood.close().await;
    fixture.wait_closed(victim_id).await?;
    assert!(!fixture.session.is_closed());
    assert_eq!(
        fixture
            .session
            .exec("healthy after cancelled close")
            .await?
            .stdout,
        b"healthy after cancelled close"
    );
    fixture.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn dropping_completed_oneshot_handoff_closes_the_owned_channel() -> Result<(), Box<dyn Error>>
{
    let fixture = Fixture::new().await?;
    let channel = open_session(&fixture.session).await?;
    let id = channel_id(&channel);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    assert!(sender.send(channel).is_ok());
    // Model cancellation after the independent opener sent a result but before
    // the waiting caller could take ownership of the completed oneshot payload.
    drop(receiver);
    fixture.wait_closed(id).await?;
    assert!(!fixture.session.is_closed());
    assert_eq!(
        fixture.session.exec("healthy after handoff").await?.stdout,
        b"healthy after handoff"
    );
    fixture.session.close().await?;
    Ok(())
}

#[tokio::test]
async fn failed_close_after_protocol_exit_still_shuts_down_the_shared_socket()
-> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new().await?;
    let channel = open_session(&fixture.session).await?;
    fixture
        .session
        .handle
        .disconnect(russh::Disconnect::ByApplication, "stop protocol", "en")
        .await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while !fixture.session.handle.is_closed() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert!(
        !fixture.session.transport.is_closed(),
        "protocol exit does not release the duplicate socket owner"
    );
    channel.close().await;
    assert!(
        fixture.session.transport.is_closed(),
        "failed CLOSE must force the physical socket shutdown even after protocol exit"
    );
    Ok(())
}
