//! Opt-in real-model evaluation using production prompts, source lookup and storage.
use super::*;
use crate::config::{AppConfig, CliConfig, FileConfig, MetadataEnrichmentJobSettings};
use crate::enrichment_store::SqliteEnrichmentStore;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

#[derive(Deserialize)]
struct Suite {
    version: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    title: String,
    performer: String,
    composer: Option<String>,
    metadata: Option<serde_json::Value>,
    group: Option<String>,
    qid: Option<String>,
    #[serde(default)]
    creators: Vec<String>,
    #[serde(default)]
    title_terms: Vec<String>,
    kind: Option<String>,
    #[serde(default)]
    forbidden_qids: Vec<String>,
}

fn suite() -> Suite {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/work-evaluation-v1.json"
    ))
    .unwrap()
}

fn context(case: &Case) -> serde_json::Value {
    let mut artists =
        vec![json!({"artist":{"id":"fixture-performer","name":case.performer},"role":0})];
    if let Some(composer) = &case.composer {
        artists.push(json!({"artist":{"id":"fixture-composer","name":composer},"role":2}));
    }
    json!({"track":{
        "track":{"id":case.id,"name":case.title,"album_id":"fixture-album","disc_number":1,"track_number":1,"duration_ms":180000,"explicit":false,"popularity":0},
        "album":{"id":"fixture-album","name":"Evaluation fixture"},"artists":artists,
    },"metadata":case.metadata})
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn identity_errors(case: &Case, work: Option<&crate::enrichment_store::Work>) -> Vec<String> {
    let mut errors = Vec::new();
    match (&case.group, work) {
        (None, Some(_)) => errors.push("false_creation".into()),
        (Some(_), None) => errors.push("missed_identity".into()),
        (Some(_), Some(work)) => {
            let names =
                |values: &[String]| values.iter().map(|n| normalize(n)).collect::<BTreeSet<_>>();
            if names(&case.creators) != names(&work.creators) {
                errors.push("wrong_creators".into());
            }
            if case
                .title_terms
                .iter()
                .any(|t| !normalize(&work.title).contains(&normalize(t)))
            {
                errors.push("wrong_title_or_granularity".into());
            }
            if case.kind.as_ref().is_some_and(|k| k != &work.kind) {
                errors.push("wrong_kind".into());
            }
            if work.wikidata_id.as_ref().is_some_and(|id| {
                case.forbidden_qids.contains(id)
                    || case.qid.as_ref().is_some_and(|expected| expected != id)
            }) {
                errors.push("wrong_external_identity".into());
            }
        }
        (None, None) => {}
    }
    errors
}

#[test]
fn work_evaluation_cases_have_unique_ids_and_hide_expected_labels() {
    let suite = suite();
    let mut ids = BTreeSet::new();
    for case in &suite.cases {
        assert!(ids.insert(&case.id));
        assert_eq!(case.group.is_some(), !case.creators.is_empty());
        let context = context(case);
        assert!(context.get("group").is_none());
        assert!(context.get("qid").is_none());
        assert!(context.get("creators").is_none());
    }
    assert_eq!(suite.cases.len(), 16);
}

#[test]
fn work_evaluation_scoring_rejects_wrong_and_unsupported_identities() {
    let suite = suite();
    let case = &suite.cases[0];
    let mut work = crate::enrichment_store::Work {
        musicbrainz_id: None,
        id: "test-work".into(),
        title: case.title.clone(),
        creators: case.creators.clone(),
        catalog_number: None,
        kind: "song".into(),
        created_at: 0,
        wikidata_id: case.qid.clone(),
    };
    assert!(identity_errors(case, Some(&work)).is_empty());
    work.creators.reverse();
    assert!(identity_errors(case, Some(&work)).is_empty());
    work.wikidata_id = Some("Q1603052".into());
    assert!(identity_errors(case, Some(&work)).contains(&"wrong_external_identity".into()));
    assert_eq!(identity_errors(case, None), ["missed_identity"]);
    let ambiguous = suite
        .cases
        .iter()
        .find(|c| c.id == "ambiguous-title")
        .unwrap();
    assert_eq!(identity_errors(ambiguous, Some(&work)), ["false_creation"]);
    assert!(identity_errors(ambiguous, None).is_empty());
    let movement = suite
        .cases
        .iter()
        .find(|c| c.id == "moonlight-first")
        .unwrap();
    work.title = "Piano Sonata No. 14".into();
    work.creators = movement.creators.clone();
    work.wikidata_id = Some("Q12008".into());
    let errors = identity_errors(movement, Some(&work));
    for expected in [
        "wrong_title_or_granularity",
        "wrong_kind",
        "wrong_external_identity",
    ] {
        assert!(errors.contains(&expected.into()));
    }
}

#[tokio::test]
#[ignore = "requires WORK_EVAL_CONFIG and WORK_EVAL_REPORT; calls the configured model and Wikidata"]
async fn work_evaluation_real_model() {
    let path = std::env::var("WORK_EVAL_CONFIG")
        .expect("set WORK_EVAL_CONFIG to the active server TOML config");
    let report =
        std::env::var("WORK_EVAL_REPORT").expect("set WORK_EVAL_REPORT to a new JSONL report path");
    let repeats: usize = std::env::var("WORK_EVAL_REPEATS")
        .unwrap_or_else(|_| "2".into())
        .parse()
        .unwrap();
    assert!((1..=5).contains(&repeats));
    let config =
        FileConfig::load(std::path::Path::new(&path)).expect("cannot load evaluation config");
    assert!(
        config
            .agent
            .as_ref()
            .and_then(|a| a.llm.as_ref())
            .and_then(|llm| llm.model.as_ref())
            .is_some(),
        "config must explicitly identify the actual model"
    );
    let temp = tempfile::TempDir::new().unwrap();
    // Resolve only the agent section using the application's own defaults and
    // validation. No production catalog, server DB or job queue is opened.
    let app = AppConfig::resolve(
        &CliConfig {
            db_dir: Some(temp.path().into()),
            ..Default::default()
        },
        Some(FileConfig {
            agent: config.agent,
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(app.agent.enabled, "configured enrichment agent is disabled");
    let model = super::super::metadata_enrichment::build_provider(&app.agent);
    let job =
        MetadataEnrichmentJob::from_settings(&MetadataEnrichmentJobSettings::default(), app.agent);
    let suite = suite();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report)
        .expect("report path must be new and writable");
    writeln!(file,"{}",json!({"suite":suite.version,"prompt_version":PROMPT_VERSION,"provider":model.name(),"model":model.model(),"repeats":repeats,"cases":suite.cases.len()})).unwrap();
    let mut failed = 0;
    let mut total = 0;
    let mut prior = BTreeMap::new();
    for repetition in 0..repeats {
        let store = SqliteEnrichmentStore::new(
            temp.path().join(format!("eval-{repetition}.db")),
            &crate::backup::DbRegistry::new(),
        )
        .unwrap();
        let mut groups = BTreeMap::<String, String>::new();
        let mut identities = BTreeMap::<String, String>::new();
        let cases: Vec<_> = if repetition % 2 == 0 {
            suite.cases.iter().collect()
        } else {
            suite.cases.iter().rev().collect()
        };
        for case in cases {
            total += 1;
            let started = std::time::Instant::now();
            let result = job
                .identify_work_context(&store, model.as_ref(), context(case))
                .await;
            let record = match result {
                Ok(evaluation) => {
                    let resolution = store
                        .resolve_track_work(
                            &case.id,
                            evaluation.identification.work.as_ref(),
                            &evaluation.evidence,
                            &evaluation.identification.reason,
                        )
                        .unwrap();
                    let mut errors = identity_errors(case, resolution.work.as_ref());
                    if let (Some(group), Some(work)) = (&case.group, &resolution.work) {
                        if groups.get(group).is_some_and(|id| id != &work.id) {
                            errors.push("duplicate_work".into());
                        }
                        if identities.get(&work.id).is_some_and(|g| g != group) {
                            errors.push("false_merge".into());
                        }
                        groups.insert(group.clone(), work.id.clone());
                        identities.insert(work.id.clone(), group.clone());
                    }
                    let signature = resolution.work.as_ref().map(|w| {
                        let creators = w.creators.iter().map(|n| normalize(n)).collect::<BTreeSet<_>>();
                        json!({"title":normalize(&w.title),"creators":creators,"catalog_number":w.catalog_number.as_deref().map(normalize),"kind":w.kind,"wikidata_id":w.wikidata_id})
                    });
                    if prior.get(&case.id).is_some_and(|old| old != &signature) {
                        errors.push("inconsistent_repeat".into());
                    }
                    prior.insert(case.id.clone(), signature);
                    json!({"case":case.id,"repetition":repetition,"errors":errors,"resolution":resolution,"evaluation":evaluation,"elapsed_ms":started.elapsed().as_millis()})
                }
                Err(error) => {
                    json!({"case":case.id,"repetition":repetition,"errors":["pipeline_error"],"error":format!("{error:?}"),"elapsed_ms":started.elapsed().as_millis()})
                }
            };
            if !record["errors"].as_array().unwrap().is_empty() {
                failed += 1;
            }
            println!(
                "EVAL {} repeat {}: {}",
                case.id, repetition, record["errors"]
            );
            writeln!(file, "{record}").unwrap();
            file.flush().unwrap();
        }
    }
    writeln!(
        file,
        "{}",
        json!({"summary":{"total":total,"failed":failed,"passed":total-failed}})
    )
    .unwrap();
    assert_eq!(
        failed, 0,
        "evaluation failed; inspect the JSONL evidence before changing prompts or thresholds"
    );
}
