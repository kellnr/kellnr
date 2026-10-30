//! Per-username throttling of password authentication attempts.
//!
//! Kellnr often runs behind a reverse proxy, where every request shares the
//! proxy's IP address. Throttling by IP would lock out all users at once, so
//! attempts are counted per username instead. Unknown usernames are counted
//! the same way, so the throttle does not reveal which users exist.

use std::hash::{BuildHasher, RandomState};
use std::time::{Duration, Instant};

use moka::future::Cache;

/// Upper bound of usernames tracked at once. Bounds memory when an attacker
/// sprays random usernames; the least valuable entries are evicted first.
/// Usernames are tracked by their hash, so long usernames do not take more
/// memory.
const MAX_TRACKED_USERS: u64 = 100_000;

#[derive(Clone, Copy)]
struct AttemptWindow {
    attempts: u32,
    started: Instant,
}

pub struct LoginThrottle {
    max_attempts: u32,
    window: Duration,
    attempts: Option<Cache<u64, AttemptWindow>>,
    /// Randomly seeded, so attackers cannot craft usernames that share the
    /// hash of a victim.
    hasher: RandomState,
}

impl LoginThrottle {
    /// Allows `max_attempts` password checks per user within `window`. A
    /// successful check resets the user's count. Zero disables throttling.
    pub fn new(max_attempts: u32, window: Duration) -> Self {
        let attempts = (max_attempts > 0 && !window.is_zero()).then(|| {
            Cache::builder()
                .max_capacity(MAX_TRACKED_USERS)
                .time_to_live(window)
                .build()
        });
        Self {
            max_attempts,
            window,
            attempts,
            hasher: RandomState::new(),
        }
    }

    fn key(&self, user: &str) -> u64 {
        self.hasher.hash_one(user)
    }

    /// Registers a password check for `user` before it runs. Returns `false`
    /// if the user exhausted the allowed attempts in the current window, in
    /// which case the password must not be checked at all.
    ///
    /// Counting before the check (instead of after a failure) keeps parallel
    /// requests from exceeding the limit.
    pub async fn try_attempt(&self, user: &str) -> bool {
        let Some(attempts) = &self.attempts else {
            return true;
        };

        let window = self.window;
        // Saturate one above the limit so the count cannot overflow.
        let ceiling = self.max_attempts.saturating_add(1);
        let entry = attempts
            .entry(self.key(user))
            .and_upsert_with(|existing| {
                let mut current = existing
                    .map(moka::Entry::into_value)
                    .filter(|w| w.started.elapsed() < window)
                    .unwrap_or(AttemptWindow {
                        attempts: 0,
                        started: Instant::now(),
                    });
                current.attempts = current.attempts.saturating_add(1).min(ceiling);
                std::future::ready(current)
            })
            .await;

        entry.into_value().attempts <= self.max_attempts
    }

    /// Clears the attempts of `user` after a successful password check.
    pub async fn succeeded(&self, user: &str) {
        if let Some(attempts) = &self.attempts {
            attempts.invalidate(&self.key(user)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Duration = Duration::from_mins(5);

    async fn exhaust(throttle: &LoginThrottle, user: &str, n: u32) {
        for _ in 0..n {
            assert!(throttle.try_attempt(user).await);
        }
    }

    #[tokio::test]
    async fn allows_attempts_up_to_the_limit() {
        let throttle = LoginThrottle::new(3, WINDOW);

        exhaust(&throttle, "user", 3).await;

        assert!(!throttle.try_attempt("user").await);
        assert!(!throttle.try_attempt("user").await);
    }

    #[tokio::test]
    async fn locking_one_user_does_not_affect_others() {
        let throttle = LoginThrottle::new(2, WINDOW);

        exhaust(&throttle, "victim", 2).await;
        assert!(!throttle.try_attempt("victim").await);

        assert!(throttle.try_attempt("other").await);
    }

    #[tokio::test]
    async fn success_resets_only_that_user() {
        let throttle = LoginThrottle::new(2, WINDOW);

        exhaust(&throttle, "user", 2).await;
        exhaust(&throttle, "other", 2).await;
        throttle.succeeded("user").await;

        assert!(throttle.try_attempt("user").await);
        assert!(!throttle.try_attempt("other").await);
    }

    #[tokio::test]
    async fn unknown_usernames_are_throttled() {
        let throttle = LoginThrottle::new(1, WINDOW);

        assert!(throttle.try_attempt("does-not-exist").await);
        assert!(!throttle.try_attempt("does-not-exist").await);
    }

    #[tokio::test]
    async fn lock_expires_after_window() {
        let window = Duration::from_millis(100);
        let throttle = LoginThrottle::new(1, window);

        assert!(throttle.try_attempt("user").await);
        assert!(!throttle.try_attempt("user").await);

        tokio::time::sleep(window + Duration::from_millis(50)).await;

        assert!(throttle.try_attempt("user").await);
    }

    #[tokio::test]
    async fn zero_attempts_disables_throttling() {
        let throttle = LoginThrottle::new(0, WINDOW);

        exhaust(&throttle, "user", 100).await;
    }

    #[tokio::test]
    async fn zero_window_disables_throttling() {
        let throttle = LoginThrottle::new(1, Duration::ZERO);

        exhaust(&throttle, "user", 100).await;
    }

    #[tokio::test]
    async fn parallel_attempts_cannot_exceed_the_limit() {
        let throttle = std::sync::Arc::new(LoginThrottle::new(5, WINDOW));

        let handles: Vec<_> = (0..50)
            .map(|_| {
                let throttle = throttle.clone();
                tokio::spawn(async move { throttle.try_attempt("user").await })
            })
            .collect();

        let mut allowed = 0;
        for handle in handles {
            if handle.await.unwrap() {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 5);
    }
}
