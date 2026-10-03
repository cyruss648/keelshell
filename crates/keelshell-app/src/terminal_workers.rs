//! Application-owned transport threads survive a closed tab until joined.

use std::{
    io,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

pub type Completion = Arc<Mutex<Option<Result<(), String>>>>;

struct Worker {
    cancel: Arc<AtomicBool>,
    completion: Completion,
    thread: JoinHandle<()>,
}

#[derive(Default)]
pub struct Registry {
    workers: Mutex<Vec<Worker>>,
    pub quitting: AtomicBool,
}

pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Registry::default)
}

impl Registry {
    pub fn spawn(
        &self,
        name: &str,
        cancel: Arc<AtomicBool>,
        worker: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) -> io::Result<()> {
        let mut workers = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("transport registry poisoned"))?;
        if self.quitting.load(Ordering::Acquire) {
            return Err(io::Error::other("application shutdown is in progress"));
        }
        // Reap finished tabs before adding another. The view retains its own
        // completion Arc, so removing this record does not erase its UI result.
        let mut index = 0;
        while index < workers.len() {
            if workers[index].thread.is_finished() {
                let finished = workers.swap_remove(index);
                if finished.thread.join().is_err() {
                    eprintln!("Terminal shutdown: worker join failed");
                }
                if let Ok(result) = finished.completion.lock()
                    && let Some(Err(error)) = result.as_ref()
                {
                    eprintln!("Terminal shutdown: {error}");
                }
            } else {
                index += 1;
            }
        }
        let completion: Completion = Arc::new(Mutex::new(None));
        let result = completion.clone();
        let thread = thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(worker))
                    .unwrap_or_else(|_| Err("transport worker panicked".into()));
                if let Ok(mut result) = result.lock() {
                    *result = Some(outcome);
                }
            })?;
        workers.push(Worker {
            cancel,
            completion,
            thread,
        });
        Ok(())
    }

    pub fn completion(&self, cancel: &Arc<AtomicBool>) -> Option<Completion> {
        self.workers
            .lock()
            .ok()?
            .iter()
            .find(|worker| Arc::ptr_eq(&worker.cancel, cancel))
            .map(|worker| worker.completion.clone())
    }

    pub fn request_stop(&self) {
        self.quitting.store(true, Ordering::Release);
        if let Ok(workers) = self.workers.lock() {
            for worker in workers.iter() {
                worker.cancel.store(true, Ordering::Release);
            }
        }
    }

    /// Must run outside the UI thread. Stops every worker before joining any one.
    pub fn shutdown(&self) -> Vec<String> {
        self.request_stop();
        let workers = match self.workers.lock() {
            Ok(mut workers) => std::mem::take(&mut *workers),
            Err(_) => return vec!["transport registry poisoned during shutdown".into()],
        };
        let mut failures = Vec::new();
        for worker in workers {
            if worker.thread.join().is_err() {
                failures.push("transport worker join failed".into());
            }
            match worker.completion.lock() {
                Ok(mut result) => match result.take() {
                    Some(Err(error)) => failures.push(error),
                    Some(Ok(())) => {}
                    None => failures.push("transport worker returned no shutdown result".into()),
                },
                Err(_) => failures.push("transport worker completion poisoned".into()),
            }
        }
        failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_waits_for_owned_workers_and_preserves_failures() -> io::Result<()> {
        let registry = Registry::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let worker_stopped = stopped.clone();
        registry.spawn("shutdown-test", cancel, move || {
            while !worker_cancel.load(Ordering::Acquire) {
                thread::yield_now();
            }
            worker_stopped.store(true, Ordering::Release);
            Err("observable close failure".into())
        })?;
        assert_eq!(registry.shutdown(), vec!["observable close failure"]);
        assert!(stopped.load(Ordering::Acquire));
        assert!(registry.shutdown().is_empty());
        assert!(
            registry
                .spawn("late", Arc::new(AtomicBool::new(false)), || Ok(()))
                .is_err()
        );
        Ok(())
    }
}
