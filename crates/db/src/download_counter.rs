use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::version::Version;
use tracing::{info, warn};

use crate::DbProvider;

/// In-memory download counter with periodic DB flush.
///
/// Accumulates download counts in memory and writes them in batch,
/// reducing DB pressure from 3-calls-per-download to
/// 2-calls-per-unique-crate-per-flush.
pub struct DownloadCounter {
    db: Arc<dyn DbProvider>,
    counts: Mutex<HashMap<(NormalizedName, Version), u64>>,
    cached_counts: Mutex<HashMap<(NormalizedName, Version), u64>>,
}

impl DownloadCounter {
    pub fn new(db: Arc<dyn DbProvider>) -> Self {
        Self {
            db,
            counts: Mutex::new(HashMap::new()),
            cached_counts: Mutex::new(HashMap::new()),
        }
    }

    /// Record a download for a kellnr-hosted crate. Instant, no DB call.
    pub fn increment(&self, name: NormalizedName, version: Version) {
        let mut counts = self.counts.lock().expect("download counter lock poisoned");
        *counts.entry((name, version)).or_insert(0) += 1;
    }

    /// Record a download for a cached crates.io crate. Instant, no DB call.
    pub fn increment_cached(&self, name: NormalizedName, version: Version) {
        let mut counts = self
            .cached_counts
            .lock()
            .expect("cached download counter lock poisoned");
        *counts.entry((name, version)).or_insert(0) += 1;
    }

    /// Flush all accumulated counts to the database.
    pub async fn flush(&self) {
        // Swap out the current maps with empty ones to minimize lock time
        let counts = {
            let mut lock = self.counts.lock().expect("download counter lock poisoned");
            std::mem::take(&mut *lock)
        };
        let cached_counts = {
            let mut lock = self
                .cached_counts
                .lock()
                .expect("cached download counter lock poisoned");
            std::mem::take(&mut *lock)
        };

        let total_kellnr = counts.len();
        let total_cached = cached_counts.len();

        if total_kellnr == 0 && total_cached == 0 {
            return;
        }

        // Flush kellnr crate counts
        for ((name, version), count) in counts {
            if let Err(e) = self
                .db
                .increase_download_counter_by(&name, &version, count)
                .await
            {
                warn!(
                    "Failed to flush download counter for {name} {version} (count={count}): {e}"
                );
            }
        }

        // Flush cached crates.io crate counts
        for ((name, version), count) in cached_counts {
            if let Err(e) = self
                .db
                .increase_cached_download_counter_by(&name, &version, count)
                .await
            {
                warn!(
                    "Failed to flush cached download counter for {name} {version} (count={count}): {e}"
                );
            }
        }

        info!(
            "Flushed download counters: {total_kellnr} kellnr crates, {total_cached} cached crates"
        );
    }
}
