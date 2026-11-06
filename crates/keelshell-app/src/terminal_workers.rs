//! Application-owned transport threads survive a closed tab until joined.

use gpui_kit::EntityId;
use std::{
    io,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub type Completion = Arc<Mutex<Option<Result<(), String>>>>;

struct Worker {
    owner: Option<EntityId>,
    cancel: Arc<AtomicBool>,
    completion: Completion,
    thread: Option<JoinHandle<()>>,
    join_failed: bool,
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
        self.spawn_owned(None, name, cancel, worker)
    }

    pub fn spawn_owned(
        &self,
        owner: Option<EntityId>,
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
        for worker in workers.iter_mut() {
            if worker.thread.as_ref().is_some_and(JoinHandle::is_finished)
                && let Some(thread) = worker.thread.take()
            {
                worker.join_failed = thread.join().is_err();
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
            owner,
            cancel,
            completion,
            thread: Some(thread),
            join_failed: false,
        });
        Ok(())
    }

    pub fn bind_owner(&self, cancel: &Arc<AtomicBool>, owner: EntityId) {
        if let Ok(mut workers) = self.workers.lock()
            && let Some(worker) = workers
                .iter_mut()
                .find(|worker| Arc::ptr_eq(&worker.cancel, cancel))
            && worker.owner.is_none()
        {
            worker.owner = Some(owner);
        }
    }

    /// Stop and actually join only the specified window's entities. Finished
    /// records are retained until their window consumes the join receipt, so
    /// an intervening spawn cannot erase a cleanup failure.
    pub fn stop_owned(
        &self,
        owners: &[EntityId],
        deadline: Instant,
    ) -> Result<Vec<String>, String> {
        loop {
            let mut workers = self
                .workers
                .lock()
                .map_err(|_| "transport registry poisoned".to_owned())?;
            let mut pending = false;
            for worker in workers
                .iter_mut()
                .filter(|worker| worker.owner.is_some_and(|owner| owners.contains(&owner)))
            {
                worker.cancel.store(true, Ordering::Release);
                if worker.thread.as_ref().is_some_and(JoinHandle::is_finished)
                    && let Some(thread) = worker.thread.take()
                {
                    worker.join_failed = thread.join().is_err();
                }
                pending |= worker.thread.is_some();
            }
            if !pending {
                let mut failures = Vec::new();
                for worker in workers
                    .iter()
                    .filter(|worker| worker.owner.is_some_and(|owner| owners.contains(&owner)))
                {
                    if worker.join_failed {
                        return Err("transport worker join failed; owner retained".into());
                    }
                    match worker.completion.lock() {
                        Ok(result) => match result.as_ref() {
                            Some(Err(error)) => failures.push(error.clone()),
                            Some(Ok(())) => {}
                            None => return Err("transport worker has no completion receipt".into()),
                        },
                        Err(_) => return Err("transport worker completion poisoned".into()),
                    }
                }
                workers.retain(|worker| worker.owner.is_none_or(|owner| !owners.contains(&owner)));
                return Ok(failures);
            }
            drop(workers);
            if Instant::now() >= deadline {
                return Err("window worker cleanup deadline elapsed; owners retained".into());
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn request_stop_owned(&self, owners: &[EntityId]) -> Result<(), String> {
        let workers = self
            .workers
            .lock()
            .map_err(|_| "transport registry poisoned".to_owned())?;
        for worker in workers
            .iter()
            .filter(|worker| worker.owner.is_some_and(|owner| owners.contains(&owner)))
        {
            worker.cancel.store(true, Ordering::Release);
        }
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
            if worker.join_failed || worker.thread.is_some_and(|thread| thread.join().is_err()) {
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

    #[gpui_kit::test]
    fn scoped_reap_retains_finished_failure_and_never_sets_global_quitting(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext;
        let a = cx.new(|_| ());
        let b = cx.new(|_| ());
        let registry = Registry::default();
        registry
            .spawn_owned(
                Some(a.entity_id()),
                "owned-a",
                Arc::new(AtomicBool::new(false)),
                || Err("retained result".into()),
            )
            .unwrap_or_else(|error| panic!("spawn A: {error}"));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        registry
            .spawn_owned(Some(b.entity_id()), "owned-b", stop.clone(), move || {
                let end = Instant::now() + Duration::from_secs(2);
                while !worker_stop.load(Ordering::Acquire) && Instant::now() < end {
                    thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            })
            .unwrap_or_else(|error| panic!("spawn B: {error}"));
        let result = registry.stop_owned(&[a.entity_id()], Instant::now() + Duration::from_secs(1));
        assert_eq!(result, Ok(vec!["retained result".into()]));
        assert!(!stop.load(Ordering::Acquire));
        assert!(!registry.quitting.load(Ordering::Acquire));
        assert!(
            registry
                .stop_owned(&[b.entity_id()], Instant::now() + Duration::from_secs(1))
                .is_ok()
        );
    }

    #[gpui_kit::test]
    fn scoped_reap_deadline_preserves_unfinished_owner_until_actual_join(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext;
        let owner = cx.new(|_| ());
        let registry = Registry::default();
        let release = Arc::new(AtomicBool::new(false));
        let worker_release = release.clone();
        registry
            .spawn_owned(
                Some(owner.entity_id()),
                "held-cleanup",
                Arc::new(AtomicBool::new(false)),
                move || {
                    let end = Instant::now() + Duration::from_secs(2);
                    while !worker_release.load(Ordering::Acquire) && Instant::now() < end {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Ok(())
                },
            )
            .unwrap_or_else(|error| panic!("spawn held worker: {error}"));
        assert!(
            registry
                .stop_owned(
                    &[owner.entity_id()],
                    Instant::now() + Duration::from_millis(10)
                )
                .is_err()
        );
        assert!(
            registry
                .workers
                .lock()
                .is_ok_and(|workers| workers.len() == 1)
        );
        assert!(!registry.quitting.load(Ordering::Acquire));
        release.store(true, Ordering::Release);
        assert!(
            registry
                .stop_owned(
                    &[owner.entity_id()],
                    Instant::now() + Duration::from_secs(1)
                )
                .is_ok()
        );
        assert!(
            registry
                .workers
                .lock()
                .is_ok_and(|workers| workers.is_empty())
        );
    }
}
