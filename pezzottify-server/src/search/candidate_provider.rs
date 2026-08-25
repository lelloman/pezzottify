//! Internal candidate-provider and rank-fusion primitives.
//!
//! Providers deliberately return identifiers and evidence, not API models.  This
//! keeps lexical search independent from future semantic/audio implementations.

use super::HashedItemType;
use anyhow::Result;
use std::collections::HashMap;

pub const RRF_K: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Deferred is reserved for semantic/audio providers.
pub enum CandidateDelivery {
    Immediate,
    Deferred,
}

#[derive(Debug, Clone)]
pub struct RankedCandidate {
    pub item_id: String,
    pub item_type: HashedItemType,
    /// Provider-local evidence normalized to 0..=1.
    pub evidence: f64,
    pub matchable_text: String,
    /// Popularity normalized to 0..=1. It is applied only after fusion.
    pub popularity: f64,
}

#[allow(dead_code)] // The lexical provider is connection-backed; future providers implement this directly.
pub trait CandidateProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn delivery(&self) -> CandidateDelivery;
    fn weight(&self) -> f64;
    fn candidates(&self, limit: usize) -> Result<Vec<RankedCandidate>>;
}

#[derive(Debug, Clone)]
pub struct ProviderEvidence {
    #[allow(dead_code)]
    pub provider_id: String,
    #[allow(dead_code)]
    pub delivery: CandidateDelivery,
    pub weight: f64,
    pub candidates: Vec<RankedCandidate>,
}

#[derive(Debug, Clone)]
pub struct FusedCandidate {
    pub item_id: String,
    pub item_type: HashedItemType,
    pub score: f64,
    pub matchable_text: String,
}

#[derive(Default)]
pub struct CandidateCoordinator;

impl CandidateCoordinator {
    #[allow(dead_code)]
    pub fn collect(
        &self,
        providers: &[&dyn CandidateProvider],
        delivery: CandidateDelivery,
        limit: usize,
    ) -> Vec<ProviderEvidence> {
        providers
            .iter()
            .filter(|provider| provider.delivery() == delivery)
            .filter_map(|provider| {
                provider
                    .candidates(limit)
                    .ok()
                    .map(|candidates| ProviderEvidence {
                        provider_id: provider.id().to_string(),
                        delivery,
                        weight: provider.weight(),
                        candidates,
                    })
            })
            .collect()
    }

    /// Weighted reciprocal-rank fusion. Duplicate entities inside a provider
    /// contribute once, while duplicates across providers accumulate evidence.
    pub fn fuse(&self, channels: Vec<ProviderEvidence>, limit: usize) -> Vec<FusedCandidate> {
        let mut fused: HashMap<(HashedItemType, String), (f64, f64, String)> = HashMap::new();

        for channel in channels {
            let mut seen = std::collections::HashSet::new();
            for (rank, candidate) in channel.candidates.into_iter().enumerate() {
                let key = (candidate.item_type, candidate.item_id.clone());
                if !seen.insert(key.clone()) {
                    continue;
                }
                let evidence = candidate.evidence.clamp(0.0, 1.0);
                let contribution = channel.weight * evidence / (RRF_K + rank as f64 + 1.0);
                let entry = fused.entry(key).or_insert_with(|| {
                    (
                        0.0,
                        candidate.popularity.clamp(0.0, 1.0),
                        candidate.matchable_text,
                    )
                });
                entry.0 += contribution;
                entry.1 = entry.1.max(candidate.popularity.clamp(0.0, 1.0));
            }
        }

        let mut results: Vec<_> = fused
            .into_iter()
            .map(
                |((item_type, item_id), (text_score, popularity, matchable_text))| {
                    // Popularity is deliberately capped at a ten percent multiplier.
                    FusedCandidate {
                        item_id,
                        item_type,
                        score: text_score * (1.0 + 0.1 * popularity),
                        matchable_text,
                    }
                },
            )
            .collect();
        results.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.item_type.cmp(&right.item_type))
                .then_with(|| left.item_id.cmp(&right.item_id))
        });
        results.truncate(limit);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeProvider {
        id: &'static str,
        delivery: CandidateDelivery,
        weight: f64,
        fail: bool,
        candidates: Vec<RankedCandidate>,
    }

    impl CandidateProvider for FakeProvider {
        fn id(&self) -> &'static str {
            self.id
        }
        fn delivery(&self) -> CandidateDelivery {
            self.delivery
        }
        fn weight(&self) -> f64 {
            self.weight
        }
        fn candidates(&self, _limit: usize) -> Result<Vec<RankedCandidate>> {
            if self.fail {
                anyhow::bail!("provider failed")
            }
            Ok(self.candidates.clone())
        }
    }

    fn candidate(id: &str) -> RankedCandidate {
        RankedCandidate {
            item_id: id.into(),
            item_type: HashedItemType::Track,
            evidence: 1.0,
            matchable_text: id.into(),
            popularity: 0.0,
        }
    }

    #[test]
    fn fuses_duplicates_and_orders_deterministically() {
        let coordinator = CandidateCoordinator;
        let channels = vec![
            ProviderEvidence {
                provider_id: "a".into(),
                delivery: CandidateDelivery::Immediate,
                weight: 2.0,
                candidates: vec![candidate("shared"), candidate("a")],
            },
            ProviderEvidence {
                provider_id: "b".into(),
                delivery: CandidateDelivery::Deferred,
                weight: 1.0,
                candidates: vec![candidate("b"), candidate("shared"), candidate("shared")],
            },
        ];
        let results = coordinator.fuse(channels, 10);
        assert_eq!(results[0].item_id, "shared");
        assert_eq!(
            results
                .iter()
                .filter(|item| item.item_id == "shared")
                .count(),
            1
        );
    }

    #[test]
    fn deferred_failure_does_not_discard_immediate_results() {
        let immediate = FakeProvider {
            id: "immediate",
            delivery: CandidateDelivery::Immediate,
            weight: 1.0,
            fail: false,
            candidates: vec![candidate("ok")],
        };
        let deferred = FakeProvider {
            id: "deferred",
            delivery: CandidateDelivery::Deferred,
            weight: 1.0,
            fail: true,
            candidates: vec![],
        };
        let coordinator = CandidateCoordinator;
        let providers: Vec<&dyn CandidateProvider> = vec![&immediate, &deferred];
        let mut channels = coordinator.collect(&providers, CandidateDelivery::Immediate, 10);
        channels.extend(coordinator.collect(&providers, CandidateDelivery::Deferred, 10));
        assert_eq!(coordinator.fuse(channels, 10)[0].item_id, "ok");
    }
}
