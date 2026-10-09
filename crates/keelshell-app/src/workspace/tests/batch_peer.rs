//! Loopback exec responses only; received command text is recorded, never run.
use super::Checked;
use keelshell_session::{SshAuth, SshOptions, SshSession};
use russh::{
    Channel, ChannelId,
    keys::{HashAlg, PrivateKey, ssh_key::private::Ed25519Keypair},
    server,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::TcpListener,
    task::{JoinHandle, JoinSet},
};
use zeroize::Zeroizing;

struct Peer {
    channels: HashMap<ChannelId, Channel<server::Msg>>,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    code: u32,
}
impl server::Handler for Peer {
    type Error = russh::Error;
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<server::Auth, Self::Error> {
        Ok(if user == "fixture" && password == "fixture" {
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
    async fn exec_request(
        &mut self,
        id: ChannelId,
        data: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.requests
            .lock()
            .map_err(|_| russh::Error::Disconnect)?
            .push(data.to_vec());
        session.channel_success(id)?;
        session.data(id, b"fixture stdout\n\x1b[31mraw".to_vec())?;
        session.extended_data(id, 1, b"fixture stderr".to_vec())?;
        if data == b"hold" {
            return Ok(());
        }
        session.exit_status_request(id, self.code)?;
        session.eof(id)?;
        session.close(id)?;
        self.channels.remove(&id);
        Ok(())
    }
    async fn channel_close(
        &mut self,
        id: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.remove(&id);
        Ok(())
    }
}

pub(crate) struct Server {
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    pub session: SshSession,
    task: JoinHandle<()>,
}
impl Server {
    pub fn new(runtime: &tokio::runtime::Runtime, code: u32) -> Self {
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .checked("bind batch test server");
        let port = listener.local_addr().checked("batch address").port();
        let key = PrivateKey::from(Ed25519Keypair::from_seed(&[0x64; 32]));
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        let config = Arc::new(server::Config {
            keys: vec![key],
            ..Default::default()
        });
        let requests = Arc::new(Mutex::new(Vec::new()));
        let observed = requests.clone();
        let task=runtime.spawn(async move {
            let mut clients=JoinSet::new();
            loop {tokio::select!{
                accepted=listener.accept()=>{
                    let Ok((socket,_))=accepted else{break};
                    let config=config.clone();let peer=Peer{channels:HashMap::new(),requests:observed.clone(),code};
                    clients.spawn(async move{if let Ok(connection)=server::run_stream(config,socket,peer).await{let _=connection.await;}});
                }
                _=clients.join_next(),if !clients.is_empty()=>{}
            }}
        });
        let mut options = SshOptions::new("127.0.0.1", "fixture");
        options.port = port;
        options.timeout = Duration::from_secs(5);
        options.expected_host_key = Some(fingerprint);
        options.auth = SshAuth::Password(Zeroizing::new("fixture".into()));
        let session = runtime
            .block_on(SshSession::connect(options))
            .checked("connect real batch SSH");
        Self {
            requests,
            session,
            task,
        }
    }
    pub fn requests(&self) -> Vec<Vec<u8>> {
        self.requests.lock().checked("batch observations").clone()
    }
    /// Return only the number of captured requests for bounded diagnostics.
    pub fn request_count(&self) -> usize {
        self.requests.lock().checked("batch observations").len()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
