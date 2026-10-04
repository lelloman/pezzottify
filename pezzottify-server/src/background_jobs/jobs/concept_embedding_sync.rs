//! Steering concept materialization job.
//!
//! A concept is a named musical idea ("Jazz", "Piano", "Recorded in the 1960s",
//! "Composed in the 1810s", a genre tag) with a vector in every track embedding
//! namespace: the weighted mean of the unit vectors of its example tracks. Steering
//! destinations and sources can reference concepts like any catalog item.
//!
//! The job recomputes every concept weekly (or on demand through the admin jobs API),
//! writes them as `entity_embeddings` rows with `entity_type = "concept"`, and removes
//! concepts that no longer qualify. See `docs/steering-concepts.md`.

use crate::background_jobs::{
    context::JobContext,
    job::{
        BackgroundJob, JobError, JobExecutionPolicy, JobResourceClass, JobSchedule,
        ShutdownBehavior,
    },
    JobAuditLogger,
};
use crate::catalog_store::{ConceptTrackFacts, EntityEmbeddingUpsert};
use crate::config::AudioEmbeddingsSettings;
use crate::db_executor::DbPriority;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{Duration, Instant};
use tracing::info;

pub const CONCEPT_ENTITY_TYPE: &str = "concept";
const AUDIOSET_NAMESPACE: &str = "ast.audioset.v2";
const MAX_EXAMPLES: usize = 200;
const MAX_EXAMPLES_PER_ARTIST: usize = 5;
const MIN_EXAMPLES: usize = 30;
/// Minimum number of examples that must have a vector in a namespace for the concept to
/// be stored there.
const MIN_EXAMPLES_WITH_VECTOR: usize = 10;
const AUDIOSET_MIN_SCORE: f32 = 0.15;
const GENRE_TAG_MIN_TRACKS: usize = 50;
const MIN_RECORDING_YEAR: i32 = 1890;
/// Works written over a longer span are too vague to date.
const MAX_COMPOSITION_SPAN_YEARS: i32 = 40;
const STORED_EXAMPLE_IDS: usize = 20;

/// Curated AudioSet music labels by family. Labels missing from the model's label list,
/// or with fewer than `MIN_EXAMPLES` confident tracks, are skipped at run time.
const AUDIOSET_CONCEPTS: &[(&str, &[&str])] = &[
    (
        "sound_genre",
        &[
            "Pop music",
            "Hip hop music",
            "Rock music",
            "Heavy metal",
            "Punk rock",
            "Grunge",
            "Progressive rock",
            "Rock and roll",
            "Psychedelic rock",
            "Rhythm and blues",
            "Soul music",
            "Reggae",
            "Country",
            "Swing music",
            "Bluegrass",
            "Funk",
            "Folk music",
            "Middle Eastern music",
            "Jazz",
            "Disco",
            "Classical music",
            "Opera",
            "Electronic music",
            "House music",
            "Techno",
            "Dubstep",
            "Drum and bass",
            "Electronica",
            "Electronic dance music",
            "Ambient music",
            "Trance music",
            "Music of Latin America",
            "Salsa music",
            "Flamenco",
            "Blues",
            "Music for children",
            "New-age music",
            "Vocal music",
            "A capella",
            "Music of Africa",
            "Afrobeat",
            "Christian music",
            "Gospel music",
            "Music of Asia",
            "Carnatic music",
            "Music of Bollywood",
            "Ska",
            "Traditional music",
            "Independent music",
            "Theme music",
            "Soundtrack music",
            "Lullaby",
            "Video game music",
            "Christmas music",
            "Dance music",
        ],
    ),
    (
        "instrument",
        &[
            "Guitar",
            "Electric guitar",
            "Bass guitar",
            "Acoustic guitar",
            "Steel guitar, slide guitar",
            "Banjo",
            "Sitar",
            "Mandolin",
            "Zither",
            "Ukulele",
            "Piano",
            "Electric piano",
            "Organ",
            "Electronic organ",
            "Hammond organ",
            "Synthesizer",
            "Harpsichord",
            "Drum kit",
            "Drum machine",
            "Snare drum",
            "Timpani",
            "Tabla",
            "Marimba, xylophone",
            "Glockenspiel",
            "Vibraphone",
            "Steelpan",
            "Orchestra",
            "Brass instrument",
            "French horn",
            "Trumpet",
            "Trombone",
            "String section",
            "Violin, fiddle",
            "Cello",
            "Double bass",
            "Flute",
            "Saxophone",
            "Clarinet",
            "Harp",
            "Harmonica",
            "Accordion",
            "Bagpipes",
            "Didgeridoo",
            "Theremin",
            "Scratching (performance technique)",
        ],
    ),
    (
        "vocals",
        &[
            "Choir",
            "Male singing",
            "Female singing",
            "Child singing",
            "Rapping",
            "Chant",
            "Yodeling",
            "Beatboxing",
        ],
    ),
    (
        "mood",
        &[
            "Happy music",
            "Funny music",
            "Sad music",
            "Tender music",
            "Exciting music",
            "Angry music",
            "Scary music",
        ],
    ),
];

/// A concept with its example tracks and their weights.
#[derive(Debug, Clone, PartialEq)]
pub struct ConceptSpec {
    pub id: String,
    pub family: &'static str,
    pub label: String,
    pub examples: Vec<(String, f32)>,
}

/// Inputs for concept selection, already loaded from the stores.
pub struct ConceptInputs<'a> {
    pub facts: &'a [ConceptTrackFacts],
    /// AudioSet label names, in vector column order.
    pub audioset_labels: &'a [String],
    pub audioset_vectors: &'a [(String, Vec<f32>)],
    /// (track_id, first_year, last_year) from Works.
    pub composition_years: &'a [(String, i32, i32)],
}

/// Select every qualifying concept and its examples. Pure and deterministic.
pub fn select_concepts(inputs: &ConceptInputs<'_>) -> Vec<ConceptSpec> {
    let artist_of: HashMap<&str, Option<&str>> = inputs
        .facts
        .iter()
        .map(|fact| (fact.track_id.as_str(), fact.artist_id.as_deref()))
        .collect();
    let mut concepts = Vec::new();
    concepts.extend(audioset_concepts(inputs, &artist_of));
    concepts.extend(genre_tag_concepts(inputs.facts, &artist_of));
    concepts.extend(recorded_concepts(inputs.facts, &artist_of));
    concepts.extend(composed_concepts(inputs.composition_years, &artist_of));
    concepts
}

fn audioset_concepts(
    inputs: &ConceptInputs<'_>,
    artist_of: &HashMap<&str, Option<&str>>,
) -> Vec<ConceptSpec> {
    let column: HashMap<&str, usize> = inputs
        .audioset_labels
        .iter()
        .enumerate()
        .map(|(index, label)| (label.as_str(), index))
        .collect();
    let mut concepts = Vec::new();
    for (family, labels) in AUDIOSET_CONCEPTS {
        for label in *labels {
            let Some(&index) = column.get(label) else {
                continue;
            };
            let mut scored = inputs
                .audioset_vectors
                .iter()
                .filter(|(track_id, _)| artist_of.contains_key(track_id.as_str()))
                .filter_map(|(track_id, vector)| {
                    let score = *vector.get(index)?;
                    (score >= AUDIOSET_MIN_SCORE).then_some((track_id.as_str(), score))
                })
                .collect::<Vec<_>>();
            scored.sort_by(|left, right| {
                right
                    .1
                    .partial_cmp(&left.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| left.0.cmp(right.0))
            });
            let examples = cap_per_artist(
                scored.into_iter().map(|(track_id, _)| (track_id, 1.0)),
                artist_of,
            );
            if examples.len() >= MIN_EXAMPLES {
                concepts.push(ConceptSpec {
                    id: format!("audioset:{label}"),
                    family,
                    label: (*label).to_string(),
                    examples,
                });
            }
        }
    }
    concepts
}

fn genre_tag_concepts(
    facts: &[ConceptTrackFacts],
    artist_of: &HashMap<&str, Option<&str>>,
) -> Vec<ConceptSpec> {
    let mut by_tag: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for fact in facts {
        let mut seen = HashSet::new();
        for genre in &fact.genres {
            if seen.insert(genre.as_str()) {
                by_tag
                    .entry(genre.as_str())
                    .or_default()
                    .push(fact.track_id.as_str());
            }
        }
    }
    by_tag
        .into_iter()
        .filter(|(_, tracks)| tracks.len() >= GENRE_TAG_MIN_TRACKS)
        .filter_map(|(tag, tracks)| {
            let examples = cap_per_artist(
                deterministic_order(tag, tracks).map(|track_id| (track_id, 1.0)),
                artist_of,
            );
            (examples.len() >= MIN_EXAMPLES).then(|| ConceptSpec {
                id: format!("genre:{tag}"),
                family: "genre_tag",
                label: tag.to_string(),
                examples,
            })
        })
        .collect()
}

fn recorded_concepts(
    facts: &[ConceptTrackFacts],
    artist_of: &HashMap<&str, Option<&str>>,
) -> Vec<ConceptSpec> {
    let mut by_decade: BTreeMap<i32, Vec<&str>> = BTreeMap::new();
    for fact in facts {
        if let Some(year) = fact
            .recording_year
            .filter(|year| *year >= MIN_RECORDING_YEAR)
        {
            by_decade
                .entry(year / 10 * 10)
                .or_default()
                .push(fact.track_id.as_str());
        }
    }
    by_decade
        .into_iter()
        .filter_map(|(decade, tracks)| {
            let key = format!("recorded:{decade}s");
            let examples = cap_per_artist(
                deterministic_order(&key, tracks).map(|track_id| (track_id, 1.0)),
                artist_of,
            );
            (examples.len() >= MIN_EXAMPLES).then(|| ConceptSpec {
                id: key,
                family: "recorded",
                label: format!("Recorded in the {decade}s"),
                examples,
            })
        })
        .collect()
}

fn composed_concepts(
    composition_years: &[(String, i32, i32)],
    artist_of: &HashMap<&str, Option<&str>>,
) -> Vec<ConceptSpec> {
    // A work written across several decades contributes to each, weighted by the share
    // of its span that falls in that decade.
    let mut by_decade: BTreeMap<i32, Vec<(&str, f32)>> = BTreeMap::new();
    for (track_id, first, last) in composition_years {
        if !artist_of.contains_key(track_id.as_str()) || last - first > MAX_COMPOSITION_SPAN_YEARS {
            continue;
        }
        let span = (last - first + 1) as f32;
        let mut decade = first / 10 * 10;
        while decade <= *last {
            let overlap = (last.min(&(decade + 9)) - first.max(&decade) + 1) as f32;
            if overlap > 0.0 {
                by_decade
                    .entry(decade)
                    .or_default()
                    .push((track_id.as_str(), overlap / span));
            }
            decade += 10;
        }
    }
    by_decade
        .into_iter()
        .filter_map(|(decade, tracks)| {
            let key = format!("composed:{decade}s");
            let weights: HashMap<&str, f32> = tracks.iter().copied().collect();
            let examples = cap_per_artist(
                deterministic_order(&key, tracks.iter().map(|(track_id, _)| *track_id).collect())
                    .map(|track_id| (track_id, weights[track_id])),
                artist_of,
            );
            (examples.len() >= MIN_EXAMPLES).then(|| ConceptSpec {
                id: key,
                family: "composed",
                label: format!("Composed in the {decade}s"),
                examples,
            })
        })
        .collect()
}

/// Shuffle deterministically per concept so sampling is stable across runs but not biased
/// toward low track ids.
fn deterministic_order<'a>(salt: &str, mut tracks: Vec<&'a str>) -> impl Iterator<Item = &'a str> {
    tracks.sort_by_key(|track_id| (fnv1a(salt, track_id), *track_id));
    tracks.dedup();
    tracks.into_iter()
}

fn fnv1a(salt: &str, value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in salt.bytes().chain([0]).chain(value.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn cap_per_artist<'a>(
    candidates: impl Iterator<Item = (&'a str, f32)>,
    artist_of: &HashMap<&str, Option<&str>>,
) -> Vec<(String, f32)> {
    let mut per_artist: HashMap<&str, usize> = HashMap::new();
    let mut seen = HashSet::new();
    let mut examples = Vec::new();
    for (track_id, weight) in candidates {
        if examples.len() >= MAX_EXAMPLES || !seen.insert(track_id) {
            continue;
        }
        if let Some(artist) = artist_of.get(track_id).copied().flatten() {
            let count = per_artist.entry(artist).or_default();
            if *count >= MAX_EXAMPLES_PER_ARTIST {
                continue;
            }
            *count += 1;
        }
        examples.push((track_id.to_string(), weight));
    }
    examples
}

/// Weighted mean of the unit vectors of the examples present in `vectors`, normalised.
/// Returns the vector and how many examples contributed.
pub fn concept_vector(
    examples: &[(String, f32)],
    vectors: &HashMap<&str, &[f32]>,
) -> Option<(Vec<f32>, usize)> {
    let mut sum: Option<Vec<f32>> = None;
    let mut used = 0;
    for (track_id, weight) in examples {
        let Some(vector) = vectors.get(track_id.as_str()) else {
            continue;
        };
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if !norm.is_finite() || norm <= f32::EPSILON {
            continue;
        }
        let sum = sum.get_or_insert_with(|| vec![0.0; vector.len()]);
        if sum.len() != vector.len() {
            continue;
        }
        for (acc, value) in sum.iter_mut().zip(vector.iter()) {
            *acc += value / norm * weight;
        }
        used += 1;
    }
    let mut sum = sum?;
    let norm = sum.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return None;
    }
    for value in &mut sum {
        *value /= norm;
    }
    Some((sum, used))
}

pub struct ConceptEmbeddingSyncJob {
    settings: AudioEmbeddingsSettings,
}

impl ConceptEmbeddingSyncJob {
    pub fn new(settings: AudioEmbeddingsSettings) -> Self {
        Self { settings }
    }

    fn namespaces(&self) -> Vec<String> {
        let mut namespaces = Vec::new();
        for spec in &self.settings.specs {
            if !namespaces.contains(&spec.namespace) {
                namespaces.push(spec.namespace.clone());
            }
        }
        namespaces
    }

    fn execute_inner(&self, ctx: &JobContext) -> Result<(), JobError> {
        let audit = JobAuditLogger::new(ctx.server_db.clone(), self.id());
        let started_at = Instant::now();
        let namespaces = self.namespaces();
        audit.log_started(Some(json!({ "namespaces": namespaces })));
        let fail = |message: String| {
            audit.log_failed(&message, None);
            JobError::ExecutionFailed(message)
        };

        let facts = ctx
            .catalog_db
            .run_blocking(DbPriority::Background, |store| {
                store.list_concept_track_facts()
            })
            .map_err(|e| fail(format!("Failed to read track facts: {e}")))?;
        let audioset_vectors = ctx
            .catalog_db
            .run_blocking(DbPriority::Background, |store| {
                store.list_entity_vectors("track", AUDIOSET_NAMESPACE)
            })
            .map_err(|e| fail(format!("Failed to read {AUDIOSET_NAMESPACE} vectors: {e}")))?;
        let audioset_labels = match audioset_vectors.first() {
            Some((track_id, _)) => {
                let track_id = track_id.clone();
                ctx.catalog_db
                    .run_blocking(DbPriority::Background, move |store| {
                        store.get_entity_embedding("track", &track_id, AUDIOSET_NAMESPACE, false)
                    })
                    .map_err(|e| fail(format!("Failed to read AudioSet labels: {e}")))?
                    .and_then(|embedding| {
                        embedding.metadata["simpleAiMetadata"]["labels"]
                            .as_array()
                            .map(|labels| {
                                labels
                                    .iter()
                                    .filter_map(|label| label.as_str().map(str::to_string))
                                    .collect::<Vec<_>>()
                            })
                    })
                    .unwrap_or_default()
            }
            None => Vec::new(),
        };
        let composition_years = match &ctx.enrichment_db {
            Some(enrichment) => enrichment
                .run_blocking(DbPriority::Background, |store| {
                    store.list_track_composition_years()
                })
                .map_err(|e| fail(format!("Failed to read composition years: {e}")))?,
            None => Vec::new(),
        };
        if ctx.is_cancelled() {
            audit.log_failed("Cancelled", None);
            return Err(JobError::Cancelled);
        }

        let concepts = select_concepts(&ConceptInputs {
            facts: &facts,
            audioset_labels: &audioset_labels,
            audioset_vectors: &audioset_vectors,
            composition_years: &composition_years,
        });
        drop(audioset_vectors);
        let mut families: BTreeMap<&str, usize> = BTreeMap::new();
        for concept in &concepts {
            *families.entry(concept.family).or_default() += 1;
        }
        audit.log_progress(
            json!({ "phase": "selected", "concepts": concepts.len(), "families": families }),
        );

        let computed_at = chrono::Utc::now().timestamp();
        let mut stored: HashSet<(String, String)> = HashSet::new();
        for namespace in &namespaces {
            if ctx.is_cancelled() {
                audit.log_failed("Cancelled", None);
                return Err(JobError::Cancelled);
            }
            let ns = namespace.clone();
            let rows = ctx
                .catalog_db
                .run_blocking(DbPriority::Background, move |store| {
                    store.list_entity_vectors("track", &ns)
                })
                .map_err(|e| fail(format!("Failed to read {namespace} vectors: {e}")))?;
            let vectors: HashMap<&str, &[f32]> = rows
                .iter()
                .map(|(track_id, vector)| (track_id.as_str(), vector.as_slice()))
                .collect();
            let mut upserts = Vec::new();
            for concept in &concepts {
                let Some((vector, used)) = concept_vector(&concept.examples, &vectors) else {
                    continue;
                };
                if used < MIN_EXAMPLES_WITH_VECTOR {
                    continue;
                }
                upserts.push(EntityEmbeddingUpsert {
                    entity_type: CONCEPT_ENTITY_TYPE.to_string(),
                    entity_id: concept.id.clone(),
                    namespace: namespace.clone(),
                    vector,
                    dtype: "float32".to_string(),
                    metadata: json!({
                        "label": concept.label,
                        "family": concept.family,
                        "example_count": used,
                        "example_track_ids": concept.examples.iter()
                            .take(STORED_EXAMPLE_IDS)
                            .map(|(track_id, _)| track_id.clone())
                            .collect::<Vec<_>>(),
                        "computed_at": computed_at,
                    }),
                    model: json!({ "id": "concept-centroid", "version": 1 }),
                });
            }
            drop(vectors);
            drop(rows);
            for upsert in upserts {
                stored.insert((upsert.entity_id.clone(), upsert.namespace.clone()));
                ctx.catalog_db
                    .run_blocking(DbPriority::Background, move |store| {
                        store.upsert_entity_embedding(&upsert).map(|_| ())
                    })
                    .map_err(|e| fail(format!("Failed to store concept: {e}")))?;
            }
        }

        // Remove concepts (or namespaces of concepts) that no longer qualify.
        let existing = ctx
            .catalog_db
            .run_blocking(DbPriority::Background, |store| {
                store.list_entity_embedding_metadata(CONCEPT_ENTITY_TYPE)
            })
            .map_err(|e| fail(format!("Failed to list stored concepts: {e}")))?;
        let mut removed = 0usize;
        for (entity_id, namespace, _) in existing {
            if stored.contains(&(entity_id.clone(), namespace.clone())) {
                continue;
            }
            ctx.catalog_db
                .run_blocking(DbPriority::Background, move |store| {
                    store.delete_entity_embedding(CONCEPT_ENTITY_TYPE, &entity_id, &namespace)
                })
                .map_err(|e| fail(format!("Failed to remove stale concept: {e}")))?;
            removed += 1;
        }

        let details = json!({
            "concepts": concepts.len(),
            "families": families,
            "embeddings_stored": stored.len(),
            "embeddings_removed": removed,
            "duration_ms": started_at.elapsed().as_millis() as u64,
        });
        info!("Concept embedding sync completed: {details}");
        audit.log_completed(Some(details));
        Ok(())
    }
}

impl BackgroundJob for ConceptEmbeddingSyncJob {
    fn id(&self) -> &'static str {
        "concept_embedding_sync"
    }

    fn name(&self) -> &'static str {
        "Concept Embedding Sync"
    }

    fn description(&self) -> &'static str {
        "Recompute steering concepts (genres, instruments, moods, tags, decades) as embeddings"
    }

    fn schedule(&self) -> JobSchedule {
        JobSchedule::JitteredInterval {
            interval: Duration::from_secs(self.settings.concepts.interval_hours * 60 * 60),
            jitter: Duration::from_secs(self.settings.concepts.jitter_minutes * 60),
        }
    }

    fn execution_policy(&self) -> JobExecutionPolicy {
        JobExecutionPolicy::new(JobResourceClass::CpuBound)
            .with_queue_timeout(Duration::from_secs(60))
            .with_max_runtime(Duration::from_secs(30 * 60))
            .with_circuit_breaker(3, Duration::from_secs(60 * 60))
    }

    fn shutdown_behavior(&self) -> ShutdownBehavior {
        ShutdownBehavior::Cancellable
    }

    fn execute(&self, ctx: &JobContext) -> Result<(), JobError> {
        self.execute_inner(ctx)
    }

    fn execute_with_params(
        &self,
        ctx: &JobContext,
        _params: Option<Value>,
    ) -> Result<(), JobError> {
        self.execute_inner(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(track_id: &str, artist: &str, year: Option<i32>, genres: &[&str]) -> ConceptTrackFacts {
        ConceptTrackFacts {
            track_id: track_id.to_string(),
            artist_id: Some(artist.to_string()),
            recording_year: year,
            genres: genres.iter().map(|genre| genre.to_string()).collect(),
        }
    }

    fn labels() -> Vec<String> {
        vec!["Jazz".to_string(), "Piano".to_string(), "Dog".to_string()]
    }

    #[test]
    fn audioset_concepts_require_confident_examples_and_cap_artists() {
        // 40 artists x 2 jazz tracks; one artist has 10 strong jazz tracks.
        let mut facts = Vec::new();
        let mut vectors = Vec::new();
        for artist in 0..40 {
            for index in 0..2 {
                let id = format!("t{artist}-{index}");
                facts.push(fact(&id, &format!("a{artist}"), None, &[]));
                vectors.push((id, vec![0.5, 0.05, 0.9]));
            }
        }
        for index in 0..10 {
            let id = format!("star-{index}");
            facts.push(fact(&id, "star", None, &[]));
            vectors.push((id, vec![0.99, 0.9, 0.0]));
        }
        let labels = labels();
        let concepts = select_concepts(&ConceptInputs {
            facts: &facts,
            audioset_labels: &labels,
            audioset_vectors: &vectors,
            composition_years: &[],
        });
        let jazz = concepts.iter().find(|c| c.id == "audioset:Jazz").unwrap();
        assert_eq!(jazz.family, "sound_genre");
        // The star artist ranks first but contributes at most 5 examples.
        assert_eq!(
            jazz.examples
                .iter()
                .filter(|(id, _)| id.starts_with("star"))
                .count(),
            5
        );
        assert!(jazz.examples[0].0.starts_with("star"));
        assert_eq!(jazz.examples.len(), MAX_EXAMPLES.min(5 + 80));
        // Piano: only the star's 10 tracks are confident, capped to 5 -> below the minimum.
        assert!(concepts.iter().all(|c| c.id != "audioset:Piano"));
        // Not a curated music label.
        assert!(concepts.iter().all(|c| c.id != "audioset:Dog"));
    }

    #[test]
    fn genre_tags_and_recorded_decades_need_enough_tracks() {
        let mut facts = Vec::new();
        for index in 0..60 {
            let year = if index < 35 { 1965 } else { 1885 };
            facts.push(fact(
                &format!("t{index}"),
                &format!("a{index}"),
                Some(year),
                &["shoegaze"],
            ));
        }
        for index in 0..20 {
            facts.push(fact(
                &format!("r{index}"),
                &format!("b{index}"),
                Some(2001),
                &["rare"],
            ));
        }
        let concepts = select_concepts(&ConceptInputs {
            facts: &facts,
            audioset_labels: &[],
            audioset_vectors: &[],
            composition_years: &[],
        });
        let ids = concepts.iter().map(|c| c.id.as_str()).collect::<Vec<_>>();
        assert!(ids.contains(&"genre:shoegaze"));
        assert!(!ids.contains(&"genre:rare"));
        assert!(ids.contains(&"recorded:1960s"));
        // Too few 2000s tracks, and years before 1890 are ignored.
        assert!(!ids.contains(&"recorded:2000s"));
        assert!(!ids.iter().any(|id| id.starts_with("recorded:188")));
        let sixties = concepts.iter().find(|c| c.id == "recorded:1960s").unwrap();
        assert_eq!(sixties.label, "Recorded in the 1960s");
        assert_eq!(sixties.examples.len(), 35);
    }

    #[test]
    fn composed_decades_spread_ranges_by_overlap() {
        let mut facts = Vec::new();
        let mut years = Vec::new();
        for index in 0..40 {
            let id = format!("t{index}");
            facts.push(fact(&id, &format!("a{index}"), None, &[]));
            // 1815-1824: 5 years in the 1810s, 5 in the 1820s.
            years.push((id, 1815, 1824));
        }
        facts.push(fact("vague", "x", None, &[]));
        years.push(("vague".to_string(), 1800, 1860));
        let concepts = select_concepts(&ConceptInputs {
            facts: &facts,
            audioset_labels: &[],
            audioset_vectors: &[],
            composition_years: &years,
        });
        let tens = concepts.iter().find(|c| c.id == "composed:1810s").unwrap();
        let twenties = concepts.iter().find(|c| c.id == "composed:1820s").unwrap();
        assert!(tens.examples.iter().all(|(_, w)| (w - 0.5).abs() < 1e-6));
        assert!(twenties
            .examples
            .iter()
            .all(|(_, w)| (w - 0.5).abs() < 1e-6));
        assert!(tens.examples.iter().all(|(id, _)| id != "vague"));
        assert_eq!(tens.label, "Composed in the 1810s");
    }

    #[test]
    fn concept_vector_is_a_weighted_mean_of_unit_vectors() {
        let examples = vec![
            ("a".to_string(), 1.0),
            ("b".to_string(), 1.0),
            ("missing".to_string(), 1.0),
        ];
        let a = [10.0_f32, 0.0];
        let b = [0.0_f32, 0.5];
        let vectors: HashMap<&str, &[f32]> = [("a", &a[..]), ("b", &b[..])].into_iter().collect();
        let (vector, used) = concept_vector(&examples, &vectors).unwrap();
        assert_eq!(used, 2);
        assert!((vector[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert!((vector[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }
}
