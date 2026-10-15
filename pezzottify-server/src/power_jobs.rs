use chrono::{DateTime, Duration, TimeZone, Utc};
use chrono_tz::Europe::Rome;

pub const NIGHTLY: &[&str] = &[
    "metadata_enrichment_v1",
    "track_embedding_sync",
    "album_embedding_sync",
    "related_artists_enrichment",
    "concept_embedding_sync",
];

pub fn nightly(job: &str) -> bool {
    NIGHTLY.contains(&job)
}

/// The first 02:00 occurrence on a fold; 03:00 on the spring gap.
pub fn at_or_after(now: DateTime<Utc>) -> DateTime<Utc> {
    let mut date = now.with_timezone(&Rome).date_naive();
    loop {
        let local = date.and_hms_opt(2, 0, 0).unwrap();
        let candidate = Rome
            .from_local_datetime(&local)
            .earliest()
            .unwrap_or_else(|| {
                Rome.from_local_datetime(&date.and_hms_opt(3, 0, 0).unwrap())
                    .earliest()
                    .unwrap()
            });
        let utc = candidate.with_timezone(&Utc);
        if utc >= now {
            return utc;
        }
        date = date.succ_opt().expect("valid future calendar date");
    }
}

pub fn after_completion(job: &str, now: DateTime<Utc>) -> DateTime<Utc> {
    if job == "concept_embedding_sync" {
        at_or_after(now + Duration::days(7))
    } else {
        at_or_after(now + Duration::seconds(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn utc(value: &str) -> DateTime<Utc> {
        value.parse().unwrap()
    }

    #[test]
    fn spring_gap_moves_to_first_valid_time() {
        assert_eq!(
            at_or_after(utc("2027-03-28T00:00:00Z")),
            utc("2027-03-28T01:00:00Z")
        );
    }

    #[test]
    fn autumn_fold_runs_only_once() {
        assert_eq!(
            at_or_after(utc("2026-10-25T00:00:00Z")),
            utc("2026-10-25T00:00:00Z")
        );
        assert_eq!(
            at_or_after(utc("2026-10-25T00:30:00Z")),
            utc("2026-10-26T01:00:00Z")
        );
    }

    #[test]
    fn ordinary_nightly_completion_moves_to_next_night() {
        assert_eq!(
            after_completion("track_embedding_sync", utc("2026-10-15T00:30:00Z")),
            utc("2026-10-16T00:00:00Z")
        );
    }
}
