//! Setup admission for fixtures sharing the process-wide local mutation registry.
use super::test_server::Checked;
use std::{
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};

#[path = "fixture_observation.rs"]
mod observation;
use observation::Kind;

pub(crate) struct FixtureGroup {
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    queues: Mutex<Vec<RegisteredQueue>>,
    observation: observation::GroupId,
}

struct RegisteredQueue {
    runtime: Arc<tokio::runtime::Runtime>,
    queue: Arc<keelshell_session::sftp::TransferQueue>,
}

impl std::fmt::Debug for FixtureGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FixtureGroup")
            .finish_non_exhaustive()
    }
}

type CleanupThread = std::thread::JoinHandle<Result<(), String>>;

fn cleanup_threads() -> &'static Mutex<Vec<CleanupThread>> {
    static THREADS: OnceLock<Mutex<Vec<CleanupThread>>> = OnceLock::new();
    THREADS.get_or_init(|| Mutex::new(Vec::new()))
}

struct FixtureScope(Weak<FixtureGroup>);
impl gpui_kit::Global for FixtureScope {}

pub(crate) fn for_app(cx: &gpui_kit::App) -> Option<Arc<FixtureGroup>> {
    cx.try_global::<FixtureScope>()
        .and_then(|scope| scope.0.upgrade())
}

fn reap_finished_cleanup() {
    let finished = {
        let mut threads = cleanup_threads().lock().checked("fixture cleanup registry");
        let mut finished = Vec::new();
        let mut index = 0;
        while index < threads.len() {
            if threads[index].is_finished() {
                finished.push(threads.swap_remove(index));
            } else {
                index += 1;
            }
        }
        finished
    };
    // Never join a live thread on GPUI. A finished owner reports cleanup errors
    // before any subsequent fixture action can be mistaken for acceptance.
    for thread in finished {
        thread
            .join()
            .checked("join finished fixture cleanup thread")
            .checked("fixture queue cleanup completed");
    }
}

struct Cleanup {
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    queues: Vec<RegisteredQueue>,
    observation: observation::GroupId,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        // Failure, panic and even a failed thread spawn keep admission closed.
        // A timeout is evidence of an unfinished owner, never permission to
        // start a different test over that owner's local mutation domain.
        if let Some(permit) = self.permit.take() {
            permit.forget();
            observation::group_event(self.observation, Kind::AdmissionFailedClosed, None, 0);
        }
    }
}

impl Cleanup {
    fn finish(mut self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(120);
        observation::group_event(
            self.observation,
            Kind::CleanupStarted,
            None,
            self.queues.len(),
        );
        while let Some(RegisteredQueue { runtime, mut queue }) = self.queues.pop() {
            let queue_index = self.queues.len() + 1;
            let mut pending_observed = false;
            let queue = loop {
                queue = match Arc::try_unwrap(queue) {
                    Ok(queue) => break queue,
                    Err(queue) => queue,
                };
                if !pending_observed {
                    observation::group_event(
                        self.observation,
                        Kind::QueueReferencesPending,
                        Some(queue_index),
                        Arc::strong_count(&queue),
                    );
                    pending_observed = true;
                }
                if Instant::now() >= deadline {
                    return Err("fixture queue still has a live owner after 120 seconds".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            };
            let remaining = deadline.saturating_duration_since(Instant::now());
            observation::group_event(
                self.observation,
                Kind::QueueCloseStarted,
                Some(queue_index),
                0,
            );
            let close_result = runtime.block_on(async {
                tokio::time::timeout(remaining, queue.close())
                    .await
                    .map_err(|_| {
                        "fixture queue scheduler did not terminate in 120 seconds".to_owned()
                    })?
                    .map_err(|error| format!("fixture queue scheduler join failed: {error}"))
            });
            if close_result.is_err() {
                observation::group_event(
                    self.observation,
                    Kind::QueueCloseFailed,
                    Some(queue_index),
                    0,
                );
            }
            close_result?;
            observation::group_event(
                self.observation,
                Kind::QueueSchedulerJoined,
                Some(queue_index),
                self.queues.len(),
            );
        }
        // close() awaited each real scheduler, including its in-flight tasks.
        // Runtime/queue retention is one-way: no scheduler holds FixtureGroup.
        drop(self.permit.take());
        observation::group_event(self.observation, Kind::PermitReleased, None, 0);
        Ok(())
    }
}

impl Drop for FixtureGroup {
    fn drop(&mut self) {
        let Some(permit) = self.permit.take() else {
            return;
        };
        let queues = match self.queues.get_mut() {
            Ok(queues) => std::mem::take(queues),
            Err(_) => {
                permit.forget();
                observation::group_event(self.observation, Kind::AdmissionFailedClosed, None, 0);
                panic!("fixture queue registry poisoned; admission remains closed");
            }
        };
        observation::group_event(
            self.observation,
            Kind::GroupLastOwnerDropped,
            None,
            queues.len(),
        );
        if queues.is_empty() {
            drop(permit);
            observation::group_event(self.observation, Kind::PermitReleased, None, 0);
            return;
        }
        // The last entity or transport worker may drop on GPUI. Move the real
        // scheduler join off that thread, retaining its original runtime and
        // admission permit until all actual queue tasks have ended.
        let cleanup = Cleanup {
            permit: Some(permit),
            queues,
            observation: self.observation,
        };
        let thread = std::thread::Builder::new()
            .name("keelshell-fixture-cleanup".into())
            .spawn(move || cleanup.finish())
            .checked("start owned fixture queue cleanup");
        cleanup_threads()
            .lock()
            .checked("register owned fixture cleanup thread")
            .push(thread);
    }
}

impl FixtureGroup {
    pub(crate) fn bind_context(self: &Arc<Self>, cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            assert!(
                for_app(cx).is_none_or(|group| Arc::ptr_eq(&group, self)),
                "one actual test App shares one local fixture group"
            );
            // The App stores only a weak reference. Multiple windows/runtimes
            // within one test inherit their actual App's group, while an
            // independent App still waits for the real previous owner.
            cx.set_global(FixtureScope(Arc::downgrade(self)));
        });
    }

    pub(crate) fn acquire_for_context(
        cx: &mut gpui_kit::TestAppContext,
        runtime: &tokio::runtime::Runtime,
    ) -> Arc<Self> {
        let label = cx.test_function_name().unwrap_or("fixture_group_context");
        if let Some(group) = cx.update(|cx| for_app(cx)) {
            observation::group_event(group.observation, Kind::SameAppReused, None, 0);
            return group;
        }
        let group = runtime.block_on(Self::wait_labeled(label));
        group.bind_context(cx);
        group
    }

    pub(crate) fn acquire(runtime: &tokio::runtime::Runtime) -> Arc<Self> {
        // This is test setup, alongside the existing synchronous peer connect.
        // Admit before mount/actions, using the real Tokio clock, without entering
        // GPUI's scheduler, advancing its clock or changing its thread guards.
        runtime.block_on(Self::wait())
    }

    pub(crate) async fn wait() -> Arc<Self> {
        Self::wait_labeled("fixture_group_internal").await
    }

    async fn wait_labeled(label: &'static str) -> Arc<Self> {
        reap_finished_cleanup();
        let mut request = observation::request(label);
        static ADMISSION: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
        let admission = ADMISSION
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1)))
            .clone();
        // Existing local writers own the whole local side against possible aliases;
        // a fresh UUID directory does not isolate independent tests in one process.
        let permit_result =
            tokio::time::timeout(Duration::from_secs(120), admission.acquire_owned()).await;
        match &permit_result {
            Err(_) => {
                request.failed(Kind::AdmissionTimeout);
                observation::report();
            }
            Ok(Err(_)) => {
                request.failed(Kind::AdmissionClosed);
                observation::report();
            }
            Ok(Ok(_)) => {}
        }
        let permit = permit_result
            .checked("file fixture group admission exceeded 120 seconds")
            .checked("file fixture admission remains open");
        let observation = request.acquired();
        reap_finished_cleanup();
        let group = Arc::new(Self {
            permit: Some(permit),
            queues: Mutex::new(Vec::new()),
            observation,
        });
        observation::bound(observation, &group);
        group
    }

    pub(crate) fn retain_queue(
        &self,
        runtime: &Arc<tokio::runtime::Runtime>,
        queue: &Arc<keelshell_session::sftp::TransferQueue>,
    ) {
        let mut queues = self.queues.lock().checked("retain actual fixture queue");
        if !queues
            .iter()
            .any(|registered| Arc::ptr_eq(&registered.queue, queue))
        {
            queues.push(RegisteredQueue {
                runtime: runtime.clone(),
                queue: queue.clone(),
            });
            observation::group_event(self.observation, Kind::QueueRegistered, None, queues.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{future::Future, sync::mpsc, task::Poll};

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .checked("fixture group test runtime")
    }

    #[test]
    fn unstarted_cleanup_closure_keeps_admission_closed() {
        let admission = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = admission
            .clone()
            .try_acquire_owned()
            .checked("private failure permit");
        let cleanup = Cleanup {
            permit: Some(permit),
            queues: Vec::new(),
            observation: observation::GroupId::UNOBSERVED,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // The standard library rejects this name before starting a thread;
            // the actual captured cleanup is dropped on that failure path.
            std::thread::Builder::new()
                .name("fixture-invalid\0name".into())
                .spawn(move || cleanup.finish())
        }));
        assert!(result.is_err());
        assert_eq!(admission.available_permits(), 0);
        assert!(admission.try_acquire_owned().is_err());
    }

    #[test]
    fn registered_live_queue_keeps_admission_until_last_reference_and_scheduler_join() {
        use keelshell_session::sftp::{TransferEvent, TransferSpec};
        let runtime = Arc::new(runtime());
        let group = FixtureGroup::acquire(&runtime);
        let server = super::super::test_server::Server::new(&runtime);
        let session = server.connect(&runtime);
        let sftp = Arc::new(
            runtime
                .block_on(session.sftp())
                .checked("real registered queue SFTP"),
        );
        let bytes = vec![0x63; 144 * 1024];
        runtime
            .block_on(sftp.write("/registered-source.bin", &bytes))
            .checked("seed actual source");
        let local = tempfile::tempdir().checked("registered queue local directory");
        let path = local
            .path()
            .canonicalize()
            .checked("canonical registered fixture root")
            .join("partial.bin");
        std::fs::write(&path, &bytes[..4096]).checked("exact reviewed prefix");
        let plan = runtime
            .block_on(
                sftp.plan_file_resume(TransferSpec::download("/registered-source.bin", &path)),
            )
            .checked("actual existing local resume plan");
        let queue = Arc::new(runtime.block_on(async { sftp.clone().transfer_queue() }));
        group.retain_queue(&runtime, &queue);
        let mut job = runtime
            .block_on(queue.enqueue_resume(plan))
            .checked("real admitted continuation");
        job.pause();
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    match job.recv().await {
                        Some(TransferEvent::Paused { .. }) => break,
                        Some(TransferEvent::Queued { .. } | TransferEvent::Started { .. }) => {}
                        event => {
                            panic!("registered continuation must pause before writing: {event:?}")
                        }
                    }
                }
            })
            .await
            .checked("actual paused registered local writer");
        });
        assert_eq!(
            std::fs::read(&path).checked("paused prefix bytes"),
            bytes[..4096]
        );
        let (queued, queued_receive) = mpsc::channel();
        let (admitted, admitted_receive) = mpsc::channel();
        drop(group);
        let contender = std::thread::spawn(move || {
            let runtime = self::runtime();
            runtime.block_on(async {
                let mut wait = Box::pin(FixtureGroup::wait());
                std::future::poll_fn(|cx| {
                    assert!(wait.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
                queued
                    .send(())
                    .checked("actual queue cleanup blocks next group");
                admitted
                    .send(wait.await)
                    .checked("actual scheduler join releases group");
            });
        });
        queued_receive
            .recv_timeout(Duration::from_secs(5))
            .checked("bounded actual blocked queue owner");
        assert!(matches!(
            admitted_receive.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(job);
        // Even after cancellation, another real queue reference prevents cleanup
        // from consuming the scheduler; try_unwrap failure cannot release permit.
        assert!(matches!(
            admitted_receive.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(queue);
        let admitted = admitted_receive
            .recv_timeout(Duration::from_secs(12))
            .checked("real joined scheduler admission");
        contender
            .join()
            .checked("join actual registered queue contender");
        reap_finished_cleanup();
        runtime
            .block_on(sftp.close())
            .checked("close original registered queue SFTP");
        drop(admitted);
    }

    #[test]
    fn queued_admission_cancels_cleanly_and_waits_for_the_last_sibling() {
        let runtime = runtime();
        let parent = FixtureGroup::acquire(&runtime);
        let sibling = parent.clone();
        assert!(Arc::ptr_eq(&parent, &sibling));
        let (queued, queued_receive) = mpsc::channel();
        let (completed, completed_receive) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let runtime = self::runtime();
            runtime.block_on(async {
                let mut cancelled = Box::pin(FixtureGroup::wait());
                std::future::poll_fn(|cx| {
                    assert!(cancelled.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
                // Dropping the actual semaphore future removes its queued waiter.
                drop(cancelled);
                let mut contender = Box::pin(FixtureGroup::wait());
                std::future::poll_fn(|cx| {
                    assert!(contender.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
                queued.send(()).checked("report actual pending admission");
                let admitted = contender.await;
                completed
                    .send(admitted)
                    .checked("report released admission");
            });
        });
        queued_receive
            .recv_timeout(Duration::from_secs(5))
            .checked("bounded actual pending contender");
        assert!(matches!(
            completed_receive.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(parent);
        assert_eq!(Arc::strong_count(&sibling), 1);
        assert!(matches!(
            completed_receive.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(sibling);
        let admitted = completed_receive
            .recv_timeout(Duration::from_secs(5))
            .checked("last sibling releases the actual waiter");
        waiter.join().checked("join owned admission test thread");
        assert_eq!(Arc::strong_count(&admitted), 1);
    }
}
