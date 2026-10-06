//! Optional packet delays and monotonic observations, disabled by default.
use russh_sftp::protocol::StatusCode;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataKind {
    Lstat,
    Fstat,
    Realpath,
    ReadOpen,
    ReadClose,
    OpenDir,
    ReadDir,
}

#[derive(Clone, Debug)]
pub struct MetadataReply {
    pub kind: MetadataKind,
    pub elapsed_ms: u128,
    pub completion: bool,
}

#[derive(Default)]
pub struct MetadataCadence {
    delay_ms: AtomicUsize,
    log: Mutex<(Option<Instant>, Vec<MetadataReply>)>,
}

impl MetadataCadence {
    pub fn start(&self, delay_ms: usize) -> Result<(), &'static str> {
        *self.log.lock().map_err(|_| "metadata timeline poisoned")? =
            (Some(Instant::now()), Vec::new());
        self.delay_ms.store(delay_ms, Ordering::Release);
        Ok(())
    }
    pub fn stop(&self) {
        self.delay_ms.store(0, Ordering::Release);
    }
    pub fn timeline(&self) -> Result<Vec<MetadataReply>, &'static str> {
        Ok(self
            .log
            .lock()
            .map_err(|_| "metadata timeline poisoned")?
            .1
            .clone())
    }
    pub fn record(&self, kind: MetadataKind, completion: bool) -> Result<(), StatusCode> {
        let mut log = self.log.lock().map_err(|_| StatusCode::Failure)?;
        if let Some(epoch) = log.0 {
            log.1.push(MetadataReply {
                kind,
                elapsed_ms: epoch.elapsed().as_millis(),
                completion,
            });
        }
        Ok(())
    }
    pub async fn wait(&self, kind: MetadataKind) -> Result<(), StatusCode> {
        self.record(kind, false)?;
        let milliseconds = self.delay_ms.load(Ordering::Acquire);
        if milliseconds > 0 {
            tokio::time::sleep(Duration::from_millis(milliseconds as u64)).await;
        }
        Ok(())
    }
}
