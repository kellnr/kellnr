use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::version::Version;
use tracing::{debug, trace, warn};

use crate::DbProvider;
use crate::error::DbError;
use crate::provider::DbResult;

/// Upper bound of distinct crate versions held in memory between two flushes.
/// Counts for further versions are dropped until the next flush, so the map
/// cannot grow without bound.
const MAX_PENDING_ENTRIES: usize = 100_000;

type CountKey = (NormalizedName, Version);

/// Download counts waiting to be flushed to the database.
struct PendingCounts {
    counts: Mutex<HashMap<CountKey, u64>>,
    overflow_logged: AtomicBool,
}

impl PendingCounts {
    fn new() -> Self {
        Self {
            counts: Mutex::new(HashMap::new()),
            overflow_logged: AtomicBool::new(false),
        }
    }

    fn add(&self, key: CountKey, count: u64) {
        let mut counts = self.counts.lock().expect("download counter lock poisoned");
        if let Some(c) = counts.get_mut(&key) {
            *c += count;
        } else if counts.len() < MAX_PENDING_ENTRIES {
            counts.insert(key, count);
        } else if !self.overflow_logged.swap(true, Ordering::Relaxed) {
            warn!(
                "Download counter holds {MAX_PENDING_ENTRIES} pending entries, dropping counts for further crate versions until the next flush"
            );
        }
    }

    fn take(&self) -> HashMap<CountKey, u64> {
        let mut counts = self.counts.lock().expect("download counter lock poisoned");
        self.overflow_logged.store(false, Ordering::Relaxed);
        std::mem::take(&mut *counts)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.counts.lock().unwrap().len()
    }
}

/// In-memory download counter with periodic DB flush.
///
/// Accumulates download counts in memory and writes them in batch,
/// reducing DB pressure from 3-calls-per-download to
/// 2-calls-per-unique-crate-per-flush.
pub struct DownloadCounter {
    db: Arc<dyn DbProvider>,
    counts: PendingCounts,
    cached_counts: PendingCounts,
    flush_interval: u64,
}

impl DownloadCounter {
    pub fn new(db: Arc<dyn DbProvider>, flush_interval: u64) -> Self {
        Self {
            db,
            counts: PendingCounts::new(),
            cached_counts: PendingCounts::new(),
            flush_interval,
        }
    }

    /// Record a download for a kellnr-hosted crate.
    /// When `flush_interval` is 0, flushes directly to DB.
    pub async fn increment_and_maybe_flush(&self, name: NormalizedName, version: Version) {
        if self.flush_interval == 0 {
            if let Err(e) = self.db.increase_download_counter(&name, &version).await {
                warn!("Failed to increment download counter for {name} {version}: {e}");
            }
        } else {
            self.counts.add((name, version), 1);
        }
    }

    /// Record a download for a cached crates.io crate.
    /// When `flush_interval` is 0, flushes directly to DB.
    pub async fn increment_cached_and_maybe_flush(&self, name: NormalizedName, version: Version) {
        if self.flush_interval == 0 {
            if let Err(e) = self
                .db
                .increase_cached_download_counter(&name, &version)
                .await
            {
                warn!("Failed to increment cached download counter for {name} {version}: {e}");
            }
        } else {
            self.cached_counts.add((name, version), 1);
        }
    }

    /// Flush all accumulated counts to the database.
    pub async fn flush(&self) {
        // Swap out the current maps with empty ones to minimize lock time
        let counts = self.counts.take();
        let cached_counts = self.cached_counts.take();

        let total_kellnr = counts.len();
        let total_cached = cached_counts.len();

        if total_kellnr == 0 && total_cached == 0 {
            return;
        }

        for ((name, version), count) in counts {
            let result = self
                .db
                .increase_download_counter_by(&name, &version, count)
                .await;
            requeue_on_error(&self.counts, result, (name, version), count);
        }

        for ((name, version), count) in cached_counts {
            let result = self
                .db
                .increase_cached_download_counter_by(&name, &version, count)
                .await;
            requeue_on_error(&self.cached_counts, result, (name, version), count);
        }

        trace!(
            "Flushed download counters: {total_kellnr} kellnr crates, {total_cached} cached crates"
        );
    }
}

/// Keep the count for the next flush if writing it failed for a transient
/// reason. Counts for crates that do not exist are dropped, as retrying them
/// can never succeed.
fn requeue_on_error(pending: &PendingCounts, result: DbResult<()>, key: CountKey, count: u64) {
    let (name, version) = &key;
    match result {
        Ok(()) => {}
        Err(DbError::CrateNotFound(_)) => {
            debug!("Dropping download count for unknown crate {name} {version} (count={count})");
        }
        Err(e) => {
            warn!("Failed to flush download counter for {name} {version} (count={count}): {e}");
            pending.add(key, count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockDb;

    fn key(name: &str) -> (NormalizedName, Version) {
        (
            NormalizedName::from_unchecked_str(name),
            Version::from_unchecked_str("1.0.0"),
        )
    }

    #[tokio::test]
    async fn flush_drops_counts_for_unknown_crates() {
        let mut db = MockDb::new();
        db.expect_increase_download_counter_by()
            .times(1)
            .returning(|name, _, _| Err(DbError::CrateNotFound(name.to_string())));
        db.expect_increase_cached_download_counter_by()
            .times(1)
            .returning(|name, _, _| Err(DbError::CrateNotFound(name.to_string())));
        let counter = DownloadCounter::new(Arc::new(db), 30);
        let (name, version) = key("unknown");
        counter
            .increment_and_maybe_flush(name.clone(), version.clone())
            .await;
        counter
            .increment_cached_and_maybe_flush(name, version)
            .await;

        counter.flush().await;

        assert_eq!(0, counter.counts.len());
        assert_eq!(0, counter.cached_counts.len());
    }

    #[tokio::test]
    async fn flush_requeues_counts_on_transient_errors() {
        let mut db = MockDb::new();
        db.expect_increase_download_counter_by()
            .times(1)
            .returning(|_, _, _| Err(DbError::FailedToCountCrates));
        let counter = DownloadCounter::new(Arc::new(db), 30);
        let (name, version) = key("known");
        counter.increment_and_maybe_flush(name, version).await;

        counter.flush().await;

        assert_eq!(1, counter.counts.len());
    }

    #[test]
    fn pending_counts_are_capped() {
        let pending = PendingCounts::new();
        for i in 0..=MAX_PENDING_ENTRIES {
            pending.add(key(&format!("crate{i}")), 1);
        }
        // Existing entries keep counting even when the map is full.
        pending.add(key("crate0"), 1);

        let counts = pending.take();
        assert_eq!(MAX_PENDING_ENTRIES, counts.len());
        assert_eq!(Some(&2), counts.get(&key("crate0")));
    }
}
