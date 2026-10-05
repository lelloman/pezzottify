//! Progress and durable memory are application-owned, not model-generated summaries.
use super::*;
use sha2::{Digest, Sha256};

pub(super) const ROUNDS: usize = 5;
const CHECKPOINT_BYTES: usize = 48_000;

fn identity(context: &Value) -> Value {
    let artists: BTreeSet<_> = context["artists"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| {
            a.pointer("/artist/name")
                .or_else(|| a.get("name"))
                .and_then(Value::as_str)
        })
        .collect();
    let identity = json!({"id":context["track"]["id"],"title":context["track"]["name"],"isrc":context["track"]["external_id_isrc"],"artists":artists});
    json!(format!(
        "{:x}",
        Sha256::digest(identity.to_string().as_bytes())
    ))
}

pub(super) fn request_key(name: &str, args: &Value) -> String {
    let mut canonical = args.clone();
    if let Some(fields) = canonical.as_object_mut() {
        for value in fields.values_mut() {
            if let Some(s) = value.as_str() {
                *value = json!(s
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
                    .replace(['‘', '’'], "'"));
            }
        }
    }
    format!("{name}:{canonical}")
}

impl Evidence {
    pub(super) fn restore(context: &Value, history: &Value) -> Self {
        for attempt in history.as_array().into_iter().flatten() {
            let checkpoint = &attempt["diagnostics"]["research"]["checkpoint"];
            if checkpoint["version"] != 1
                || checkpoint["identity"] != identity(context)
                || checkpoint.to_string().len() > CHECKPOINT_BYTES
            {
                continue;
            }
            if let Ok(mut evidence) =
                serde_json::from_value::<Evidence>(checkpoint["evidence"].clone())
            {
                // Rebuild fetch eligibility from retained full documents even
                // when the separate discovered-ID list was bounded on disk.
                let documents: Vec<_> = evidence
                    .recordings
                    .values()
                    .map(|v| ("recording", v.clone()))
                    .chain(evidence.works.values().map(|v| ("work", v.clone())))
                    .collect();
                for (kind, document) in documents {
                    evidence.discover(kind, &document);
                }
                // A failed source request may recover in a later intervention.
                // Successful/empty searches remain remembered in this retry cycle.
                let successes: BTreeSet<_> = evidence
                    .trace
                    .iter()
                    .filter(|t| t["tool"].is_string() && t["result"].get("error").is_none())
                    .map(|t| request_key(t["tool"].as_str().unwrap(), &t["arguments"]))
                    .collect();
                for step in &evidence.trace {
                    if step["result"]["error"].is_string() {
                        if let Some(name) = step["tool"].as_str() {
                            let key = request_key(name, &step["arguments"]);
                            if !successes.contains(&key) {
                                evidence.requests.remove(&key);
                            }
                        }
                    }
                }
                evidence.trace.push(json!({"memory_restored":true}));
                return evidence;
            }
        }
        Self::default()
    }

    pub(super) fn checkpoint(&self, context: &Value) -> Value {
        let mut saved = json!({"known_recordings":self.known_recordings.iter().take(128).chain(self.recordings.keys()).collect::<BTreeSet<_>>(),"known_works":self.known_works.iter().take(128).chain(self.works.keys()).collect::<BTreeSet<_>>(),
            "recordings":self.recordings,"works":self.works,"requests":self.requests,
            "trace":self.trace.iter().filter(|t|t["tool"].is_string() || t["round_error"].is_string()).cloned().collect::<Vec<_>>()});
        let mut omitted = 0;
        // Never truncate a fetched document: that could hide a second work or a
        // medley qualifier. Evict whole documents and permit a fresh fetch instead.
        while saved.to_string().len() > CHECKPOINT_BYTES - 4000 {
            let largest = ["recordings", "works"]
                .into_iter()
                .flat_map(|kind| {
                    saved[kind]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(move |(id, doc)| (kind, id.clone(), doc.to_string().len()))
                })
                .max_by_key(|(_, _, size)| *size);
            if let Some((kind, id, _)) = largest {
                saved[kind].as_object_mut().unwrap().remove(&id);
                let tool = if kind == "recordings" {
                    "fetch_recording"
                } else {
                    "fetch_work"
                };
                let key = request_key(tool, &json!({"id":id}));
                saved["requests"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|v| v != &key);
            } else if !saved["trace"].as_array().unwrap().is_empty() {
                saved["trace"].as_array_mut().unwrap().remove(0);
            } else {
                break;
            }
            omitted += 1;
        }
        json!({"version":1,"identity":identity(context),"omitted_documents_or_observations":omitted,"discovered_ids_limited":self.known_recordings.len()>128 || self.known_works.len()>128,"evidence":saved})
    }

    pub(super) fn verified(&self, context: &Value) -> Option<WorkEvaluation> {
        let mut matches = BTreeMap::new();
        for (recording_id, recording) in &self.recordings {
            for work in super::super::source_knowledge::recording_works(recording) {
                if let Some(id) = work["id"].as_str() {
                    if let Ok(result) = self.verify(context, recording_id, id) {
                        matches.insert(id.to_owned(), result);
                    }
                }
            }
        }
        // Conflicting verified works still require review.
        if matches.len() == 1 {
            matches.into_values().next()
        } else {
            None
        }
    }

    /// A deterministic escape from stalled/repeated model actions. Every action
    /// must be new in the accumulated ledger, including previous interventions.
    pub(super) fn next_action(&self, context: &Value) -> Option<(&'static str, Value)> {
        let mut actions = Vec::new();
        if let Some(isrc) = context["track"]["external_id_isrc"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            actions.push(("search_recordings", json!({"isrc":isrc})));
        }
        let title = context["track"]["name"].as_str().unwrap_or_default();
        let artist = context["artists"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|a| {
                a.pointer("/artist/name")
                    .or_else(|| a.get("name"))
                    .and_then(Value::as_str)
            });
        if !title.is_empty() {
            actions.push((
                "search_recordings",
                match artist {
                    Some(a) => json!({"title":title,"artist":a}),
                    None => json!({"title":title}),
                },
            ));
        }
        // Inspect performance targets before spending calls on more discovery.
        for recording in self.recordings.values() {
            if recording_matches_catalog(recording, context) {
                for work in super::super::source_knowledge::recording_works(recording) {
                    if let Some(id) = work["id"]
                        .as_str()
                        .filter(|id| !self.works.contains_key(*id))
                    {
                        actions.push(("fetch_work", json!({"id":id})));
                    }
                }
            }
        }
        // Keep search ranking; sorted discovered IDs are only a fallback.
        for step in &self.trace {
            for row in step["result"]["recordings"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(id) = row["id"].as_str().filter(|id| {
                    self.known_recordings.contains(*id) && !self.recordings.contains_key(*id)
                }) {
                    actions.push(("fetch_recording", json!({"id":id})));
                }
            }
        }
        for id in &self.known_recordings {
            if !self.recordings.contains_key(id) {
                actions.push(("fetch_recording", json!({"id":id})));
            }
        }
        for id in &self.known_works {
            if !self.works.contains_key(id) {
                actions.push(("fetch_work", json!({"id":id})));
            }
        }
        if !title.is_empty() {
            actions.push(("search_works", json!({"title":title})));
            actions.push(("search_wikidata_works", json!({"title":title})));
        }
        actions
            .into_iter()
            .find(|(name, args)| !self.requests.contains(&request_key(name, args)))
    }

    pub(super) async fn execute(
        &mut self,
        sources: &dyn Sources,
        context: &Value,
        name: &str,
        args: &Value,
        round: usize,
        origin: &str,
    ) {
        let started = std::time::Instant::now();
        let track_id = context["track"]["id"].as_str().unwrap_or("unknown");
        tracing::info!(
            track_id,
            round,
            tool = name,
            origin,
            "Work research tool started"
        );
        let index = self.trace.len();
        self.trace.push(json!({"round":round,"origin":origin,"tool":name,"arguments":args,"result":{"error":"source request interrupted before completion"},"retrieved_at":super::super::metadata_enrichment::now_secs()}));
        let result = match self.call(sources, context, name, args).await {
            Ok(value) => value,
            Err(error) => json!({"error":format!("{error:#}")}),
        };
        tracing::info!(
            track_id,
            round,
            tool = name,
            origin,
            elapsed_ms = started.elapsed().as_millis() as u64,
            error = result["error"].as_str(),
            "Work research tool finished"
        );
        self.trace[index]["result"] = compact(&result);
    }
}

/// The full current trace remains in the Work evaluation; attempt history needs
/// the bounded checkpoint, not copies of every large prompt and model response.
pub(in super::super) fn attempt_diagnostics(evidence: &Value) -> Value {
    json!({"prompt_version":evidence["research"]["prompt_version"],"research":{
        "provider":evidence["research"]["provider"],"model":evidence["research"]["model"],
        "checkpoint":evidence["research"]["checkpoint"],"rounds":evidence["research"]["rounds"]}})
}
