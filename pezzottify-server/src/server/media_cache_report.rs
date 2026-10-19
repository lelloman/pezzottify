//! Admin snapshot from in-memory allocation accounting and existing counters.
use crate::media::cache::CacheSnapshot;
use prometheus::CounterVec;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct MediaCacheReport {
    enabled: bool,
    caches: Vec<CacheReport>,
    audio: ReadActivity,
    image: ReadActivity,
    fills_skipped_capacity: u64,
    fills_skipped_busy: u64,
}

#[derive(Serialize)]
struct CacheReport {
    #[serde(flatten)]
    allocation: CacheSnapshot,
    fills: u64,
    fill_errors: u64,
    evictions: u64,
    eviction_errors: u64,
}

#[derive(Serialize)]
struct ReadActivity {
    hits: u64,
    misses: u64,
    errors: u64,
    fallback_reads: u64,
    hit_ratio: Option<f64>,
}

pub(super) fn collect(snapshots: Option<Vec<CacheSnapshot>>) -> MediaCacheReport {
    report(snapshots, &super::metrics::MEDIA_CACHE_EVENTS)
}

fn report(snapshots: Option<Vec<CacheSnapshot>>, events: &CounterVec) -> MediaCacheReport {
    let enabled = snapshots.is_some();
    let count = |cache: &str, outcome: &str| -> u64 {
        if enabled {
            events.with_label_values(&[cache, outcome]).get() as u64
        } else {
            0
        }
    };
    let activity = |kind: &str| {
        let hits = count("all", &format!("{kind}_hit"));
        let fallback_reads = count("all", &format!("{kind}_fallback_read"));
        let requests = hits.saturating_add(fallback_reads);
        ReadActivity {
            hits,
            misses: count("all", &format!("{kind}_miss")),
            errors: count("all", &format!("{kind}_error")),
            fallback_reads,
            hit_ratio: (requests > 0).then(|| hits as f64 / requests as f64),
        }
    };
    MediaCacheReport {
        enabled,
        caches: snapshots
            .unwrap_or_default()
            .into_iter()
            .map(|allocation| CacheReport {
                fills: count(&allocation.name, "fill_completed"),
                fill_errors: count(&allocation.name, "fill_failed"),
                evictions: count(&allocation.name, "eviction"),
                eviction_errors: count(&allocation.name, "eviction_failed"),
                allocation,
            })
            .collect(),
        audio: activity("audio"),
        image: activity("image"),
        fills_skipped_capacity: count("all", "fill_skipped_capacity"),
        fills_skipped_busy: count("all", "fill_skipped_admission")
            + count("all", "fill_skipped_worker")
            + count("all", "fill_skipped_runtime"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::Opts;

    #[test]
    fn snapshot_separates_cache_allocations_from_aggregate_reads() {
        let counters = CounterVec::new(
            Opts::new("test_cache_events", "events"),
            &["cache", "outcome"],
        )
        .unwrap();
        counters
            .with_label_values(&["all", "audio_hit"])
            .inc_by(3.0);
        counters
            .with_label_values(&["all", "audio_fallback_read"])
            .inc();
        counters.with_label_values(&["all", "audio_error"]).inc();
        counters
            .with_label_values(&["ssd", "fill_completed"])
            .inc_by(2.0);
        let snapshot = CacheSnapshot {
            name: "ssd".into(),
            path: "/nonexistent/cache".into(),
            max_bytes: 100,
            allocated_bytes: 40,
            reserved_bytes: 20,
            lru: true,
        };
        let result = report(Some(vec![snapshot]), &counters);
        assert!(result.enabled);
        assert_eq!(result.audio.hit_ratio, Some(0.75));
        assert_eq!(result.audio.errors, 1);
        assert_eq!(result.image.hit_ratio, None);
        assert_eq!(result.caches[0].fills, 2);
        assert_eq!(result.caches[0].allocation.allocated_bytes, 40);
        assert_eq!(result.caches[0].allocation.reserved_bytes, 20);
        assert_eq!(result.caches[0].fill_errors, 0);
        let disabled = report(None, &counters);
        assert!(!disabled.enabled);
        assert!(disabled.caches.is_empty());
        assert_eq!(disabled.audio.hits, 0);
        assert_eq!(disabled.audio.hit_ratio, None);
    }
}
