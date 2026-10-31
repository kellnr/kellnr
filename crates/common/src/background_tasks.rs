use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{error, trace, warn};

/// Pause before a task runs again after a round panicked, unless the task
/// runs periodically, then the next round follows the interval.
const PANIC_PAUSE: Duration = Duration::from_secs(1);

/// When the first round of a periodic task runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstRound {
    Immediately,
    AfterInterval,
}

/// Background tasks that run in rounds until shutdown, or once.
///
/// Every round runs as its own tokio task. A round that panics is logged and
/// the task continues. On shutdown no new round starts, a running round or
/// one-off task is not interrupted.
#[derive(Debug, Clone, Default)]
pub struct BackgroundTasks {
    shutdown: CancellationToken,
    tracker: TaskTracker,
}

impl BackgroundTasks {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancelled on shutdown. Rounds that may run for a long time, e.g. docs
    /// builds, use it to stop early.
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    pub fn request_shutdown(&self) {
        self.shutdown.cancel();
    }

    /// Run `round` every `interval`. A round that takes longer than the
    /// interval delays the next one.
    pub fn spawn_periodic<F, Fut>(
        &self,
        name: &'static str,
        interval: Duration,
        first_round: FirstRound,
        mut round: F,
    ) where
        F: FnMut() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let first_delay = match first_round {
            FirstRound::Immediately => Duration::ZERO,
            FirstRound::AfterInterval => interval,
        };
        let timed_round = move || {
            let round = round();
            async move {
                let start = Instant::now();
                round.await;
                interval.saturating_sub(start.elapsed())
            }
        };
        self.spawn_rounds(name, first_delay, interval, timed_round);
    }

    /// Run `round` repeatedly, starting immediately. Each round returns the
    /// delay until the next one.
    pub fn spawn_loop<F, Fut>(&self, name: &'static str, round: F)
    where
        F: FnMut() -> Fut + Send + 'static,
        Fut: Future<Output = Duration> + Send + 'static,
    {
        self.spawn_rounds(name, Duration::ZERO, PANIC_PAUSE, round);
    }

    /// Run `task` once. Shutdown waits for it like for a running round.
    pub fn spawn_tracked<Fut>(&self, task: Fut) -> JoinHandle<Fut::Output>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        self.tracker.spawn(task)
    }

    /// Run `task` once and log it if it panics. Shutdown waits for it like
    /// for a running round.
    pub fn spawn_once<Fut>(&self, name: &'static str, task: Fut) -> JoinHandle<()>
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.spawn_tracked(async move {
            if let Err(e) = tokio::spawn(task).await {
                error!("Background task {name} stopped: {e}");
            }
        })
    }

    fn spawn_rounds<F, Fut>(
        &self,
        name: &'static str,
        first_delay: Duration,
        delay_after_panic: Duration,
        mut round: F,
    ) where
        F: FnMut() -> Fut + Send + 'static,
        Fut: Future<Output = Duration> + Send + 'static,
    {
        let shutdown = self.shutdown.clone();
        self.tracker.spawn(async move {
            let mut delay = first_delay;
            loop {
                tokio::select! {
                    biased;
                    () = shutdown.cancelled() => break,
                    () = tokio::time::sleep(delay) => {}
                }
                delay = match tokio::spawn(round()).await {
                    Ok(next_delay) => next_delay,
                    Err(e) if e.is_panic() => {
                        error!("Background task {name} panicked, continuing: {e}");
                        delay_after_panic
                    }
                    Err(_) => break,
                };
            }
            trace!("Background task {name} stopped");
        });
    }

    /// Wait until shutdown is requested, then up to `timeout` for the running
    /// rounds to finish.
    pub async fn wait_for_shutdown(&self, timeout: Duration) {
        self.shutdown.cancelled().await;
        self.tracker.close();
        if tokio::time::timeout(timeout, self.tracker.wait())
            .await
            .is_err()
        {
            warn!(
                "Background tasks did not finish within {}s, shutting down anyway",
                timeout.as_secs()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    const INTERVAL: Duration = Duration::from_secs(10);

    fn counting_round(
        rounds: &Arc<AtomicUsize>,
    ) -> impl FnMut() -> std::future::Ready<()> + Send + 'static {
        let rounds = rounds.clone();
        move || {
            rounds.fetch_add(1, Ordering::SeqCst);
            std::future::ready(())
        }
    }

    async fn sleep_secs(secs: u64) {
        tokio::time::sleep(Duration::from_secs(secs)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_task_runs_once_per_interval() {
        let tasks = BackgroundTasks::new();
        let rounds = Arc::new(AtomicUsize::new(0));

        tasks.spawn_periodic(
            "test",
            INTERVAL,
            FirstRound::AfterInterval,
            counting_round(&rounds),
        );

        sleep_secs(5).await;
        assert_eq!(0, rounds.load(Ordering::SeqCst));
        sleep_secs(30).await;
        assert_eq!(3, rounds.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_task_can_start_immediately() {
        let tasks = BackgroundTasks::new();
        let rounds = Arc::new(AtomicUsize::new(0));

        tasks.spawn_periodic(
            "test",
            INTERVAL,
            FirstRound::Immediately,
            counting_round(&rounds),
        );

        sleep_secs(1).await;
        assert_eq!(1, rounds.load(Ordering::SeqCst));
        sleep_secs(10).await;
        assert_eq!(2, rounds.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_task_continues_after_a_panicking_round() {
        let tasks = BackgroundTasks::new();
        let rounds = Arc::new(AtomicUsize::new(0));
        let counter = rounds.clone();

        tasks.spawn_periodic("test", INTERVAL, FirstRound::Immediately, move || {
            let round = counter.fetch_add(1, Ordering::SeqCst);
            async move {
                assert!(round != 0, "first round panics");
            }
        });

        sleep_secs(25).await;
        assert_eq!(3, rounds.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn loop_task_waits_the_delay_returned_by_each_round() {
        let tasks = BackgroundTasks::new();
        let rounds = Arc::new(AtomicUsize::new(0));
        let counter = rounds.clone();

        tasks.spawn_loop("test", move || {
            let round = counter.fetch_add(1, Ordering::SeqCst);
            async move { Duration::from_secs(if round == 0 { 100 } else { 10 }) }
        });

        sleep_secs(99).await;
        assert_eq!(1, rounds.load(Ordering::SeqCst));
        sleep_secs(20).await;
        assert_eq!(3, rounds.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn loop_task_pauses_after_a_panicking_round() {
        let tasks = BackgroundTasks::new();
        let rounds = Arc::new(AtomicUsize::new(0));
        let counter = rounds.clone();

        tasks.spawn_loop("test", move || {
            let round = counter.fetch_add(1, Ordering::SeqCst);
            async move {
                assert!(round != 0, "first round panics");
                Duration::from_secs(100)
            }
        });

        sleep_secs(2).await;
        assert_eq!(2, rounds.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_lets_the_running_round_finish_and_stops_the_task() {
        let tasks = BackgroundTasks::new();
        let started = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        let (started_in_round, finished_in_round) = (started.clone(), finished.clone());

        tasks.spawn_periodic("test", INTERVAL, FirstRound::Immediately, move || {
            started_in_round.fetch_add(1, Ordering::SeqCst);
            let finished = finished_in_round.clone();
            async move {
                sleep_secs(5).await;
                finished.fetch_add(1, Ordering::SeqCst);
            }
        });

        sleep_secs(1).await;
        tasks.request_shutdown();
        tasks.wait_for_shutdown(Duration::from_secs(30)).await;

        assert_eq!(1, finished.load(Ordering::SeqCst));
        sleep_secs(60).await;
        assert_eq!(1, started.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_waits_for_a_task_spawned_once() {
        let tasks = BackgroundTasks::new();
        let finished = Arc::new(AtomicUsize::new(0));
        let finished_in_task = finished.clone();

        tasks.spawn_once("test", async move {
            sleep_secs(5).await;
            finished_in_task.fetch_add(1, Ordering::SeqCst);
        });

        tasks.request_shutdown();
        tasks.wait_for_shutdown(Duration::from_secs(30)).await;

        assert_eq!(1, finished.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn task_spawned_once_logs_a_panic_instead_of_passing_it_on() {
        let tasks = BackgroundTasks::new();

        tasks
            .spawn_once("test", async { panic!("task panics") })
            .await
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn wait_for_shutdown_gives_up_after_the_timeout() {
        let tasks = BackgroundTasks::new();
        tasks.spawn_loop("test", || async {
            sleep_secs(3600).await;
            Duration::ZERO
        });

        sleep_secs(1).await;
        tasks.request_shutdown();
        let start = Instant::now();
        tasks.wait_for_shutdown(Duration::from_secs(30)).await;

        assert_eq!(Duration::from_secs(30), start.elapsed());
    }
}
