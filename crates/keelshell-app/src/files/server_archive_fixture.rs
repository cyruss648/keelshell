//! Execute only explicitly registered production helpers on owned fixture paths.
#[cfg(unix)]
use super::Checked;
use super::filesystem;
use russh::{Channel, ChannelMsg, server};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) struct Recipe {
    command: String,
    mirror: Option<PathBuf>,
}

#[derive(Default)]
pub(in crate::files) struct Fixture {
    recipes: Mutex<HashMap<Vec<u8>, Recipe>>,
    pub(in crate::files) calls: AtomicUsize,
    inputs: Mutex<Vec<Vec<u8>>>,
    hold: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

impl Fixture {
    #[cfg(unix)]
    pub(in crate::files) fn allow(&self, command: String, mirror: Option<PathBuf>) {
        self.recipes
            .lock()
            .checked("fixture recipes")
            .insert(command.as_bytes().to_vec(), Recipe { command, mirror });
    }

    pub(super) fn lookup(&self, command: &[u8]) -> Option<Recipe> {
        let recipe = self.recipes.lock().ok()?.get(command).cloned()?;
        self.calls.fetch_add(1, Ordering::AcqRel);
        Some(recipe)
    }

    #[cfg(unix)]
    pub(in crate::files) fn inputs(&self) -> Vec<Vec<u8>> {
        self.inputs.lock().checked("fixture inputs").clone()
    }

    #[cfg(unix)]
    pub(in crate::files) fn hold_next(&self) -> tokio::sync::oneshot::Sender<()> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        *self.hold.lock().checked("fixture hold") = Some(receiver);
        sender
    }

    pub(super) async fn serve(
        &self,
        mut channel: Channel<server::Msg>,
        recipe: Recipe,
        mut filesystem: filesystem::Filesystem,
    ) {
        let input = tokio::time::timeout(Duration::from_secs(7), async {
            let mut input = Vec::new();
            while let Some(message) = channel.wait().await {
                match message {
                    ChannelMsg::Data { data } => {
                        if input.len().saturating_add(data.len()) > 1024 * 1024 {
                            return None;
                        }
                        input.extend_from_slice(&data);
                    }
                    ChannelMsg::Eof => return Some(input),
                    ChannelMsg::Close => return None,
                    _ => {}
                }
            }
            None
        })
        .await;
        let Ok(Some(input)) = input else {
            let _ = channel.close().await;
            return;
        };
        if let Ok(mut inputs) = self.inputs.lock() {
            inputs.push(input.clone());
        }
        let hold = self.hold.lock().ok().and_then(|mut hold| hold.take());
        if let Some(hold) = hold
            && tokio::time::timeout(Duration::from_secs(7), hold)
                .await
                .is_err()
        {
            let _ = channel.close().await;
            return;
        }
        let mirror = recipe.mirror.clone();
        let result = tokio::task::spawn_blocking(move || run(recipe, &input)).await;
        let (stdout, stderr, mut status, bytes) = match result {
            Ok(Ok(result)) => result,
            _ => (Vec::new(), b"fixture helper failed".to_vec(), 70, None),
        };
        if status == 0
            && let (Some(path), Some(bytes)) = (mirror, bytes)
        {
            use russh_sftp::{protocol::FileAttributes, server::Handler};
            // The real Python helper wrote the owned host file. Mirror that
            // external publication into the deterministic packet filesystem;
            // this is fixture coordination, not another application write.
            let published = path
                .to_str()
                .is_some_and(|path| filesystem.replace_external_file(path, &bytes).is_ok());
            let mut attrs = FileAttributes::empty();
            attrs.permissions = Some(0o600);
            if !published
                || filesystem
                    .setstat(1, path.to_string_lossy().into_owned(), attrs)
                    .await
                    .is_err()
            {
                status = 70;
            }
        }
        let _ = channel.data(&stdout[..]).await;
        if !stderr.is_empty() {
            let _ = channel.extended_data(1, &stderr[..]).await;
        }
        let _ = channel.exit_status(status).await;
        let _ = channel.eof().await;
        let _ = channel.close().await;
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

type HelperOutput = (Vec<u8>, Vec<u8>, u32, Option<Vec<u8>>);

fn run(recipe: Recipe, input: &[u8]) -> std::io::Result<HelperOutput> {
    let mut stdin = tempfile::tempfile()?;
    stdin.write_all(input)?;
    stdin.seek(SeekFrom::Start(0))?;
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    // exec replaces the shell with Python so the bounded process owner never
    // leaves a detached interpreter after a failed or timed-out fixture.
    let mut process = Process(
        Command::new("sh")
            .arg("-c")
            .arg(format!("exec {}", recipe.command))
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()?,
    );
    let until = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = process.0.try_wait()? {
            break status;
        }
        if Instant::now() >= until
            || stdout
                .metadata()?
                .len()
                .saturating_add(stderr.metadata()?.len())
                > 1024 * 1024
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "fixture helper exceeded its envelope",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    stdout.seek(SeekFrom::Start(0))?;
    stderr.seek(SeekFrom::Start(0))?;
    let mut output = Vec::new();
    let mut errors = Vec::new();
    stdout.take(1024 * 1024 + 1).read_to_end(&mut output)?;
    stderr.take(1024 * 1024 + 1).read_to_end(&mut errors)?;
    if output.len().saturating_add(errors.len()) > 1024 * 1024 {
        return Err(std::io::Error::other("fixture helper output limit"));
    }
    let bytes = if status.success() {
        if let Some(path) = &recipe.mirror {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(keelshell_core::MAX_ARCHIVE_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > keelshell_core::MAX_ARCHIVE_BYTES {
                return Err(std::io::Error::other("fixture archive byte limit"));
            }
            Some(bytes)
        } else {
            None
        }
    } else {
        None
    };
    Ok((output, errors, status.code().unwrap_or(70) as u32, bytes))
}
