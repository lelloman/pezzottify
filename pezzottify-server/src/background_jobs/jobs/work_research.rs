//! A bounded, read-only research loop. Only deterministic source validation can link.
use super::source_knowledge::{recording_matches_catalog, valid_mbid, ReferenceClient};
use super::work_knowledge::{WikidataWorkLookup, WorkKnowledgeLookup};
use super::work_resolution::{musicbrainz_work_evaluation, Identification, WorkEvaluation};
use crate::agent::{llm::FinishReason, CompletionOptions, LlmProvider, Message, ToolDefinition};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[path = "work_research_loop.rs"]
mod progress;
#[path = "work_research_state.rs"]
mod state;
pub(super) use progress::attempt_diagnostics;

#[async_trait::async_trait]
trait Sources: Send + Sync {
    async fn search(&self, kind: &str, query: &str) -> Result<Value>;
    async fn fetch(&self, kind: &str, id: &str) -> Result<Value>;
    async fn wikidata(&self, title: &str) -> Result<Value>;
}

#[async_trait::async_trait]
impl Sources for ReferenceClient {
    async fn search(&self, kind: &str, query: &str) -> Result<Value> {
        self.research_search(kind, query).await
    }
    async fn fetch(&self, kind: &str, id: &str) -> Result<Value> {
        self.research_fetch(kind, id).await
    }
    async fn wikidata(&self, title: &str) -> Result<Value> {
        Ok(serde_json::to_value(
            WikidataWorkLookup.lookup(title).await?,
        )?)
    }
}

fn tools() -> Vec<ToolDefinition> {
    let mut out = vec![ToolDefinition::new("search_recordings", "Search recordings using a catalog ISRC or title/artist. Supply plain text, not search syntax. The application handles the initial ISRC and title/artist searches. Choose an untried query from the current state.",json!({"type":"object","properties":{"isrc":{"type":"string","maxLength":32},"title":{"type":"string","maxLength":200},"artist":{"type":"string","maxLength":200}},"minProperties":1,"additionalProperties":false}))];
    for (name, field, description) in [
        ("search_works", "title", "Search MusicBrainz compositions by plain-text title. Fetch promising works for writer credits and recording relationships. A title match is not a verified link."),
        ("fetch_recording", "id", "Fetch a discovered recording with ISRC, performer credits and performance relationships."),
        ("fetch_work", "id", "Fetch a discovered work with writer credits and recording relationships."),
        ("search_wikidata_works", "title", "Find composition candidates and creators in Wikidata; title matches require corroboration."),
    ] {
        out.push(ToolDefinition::new(name, description, json!({"type":"object","properties":{field:{"type":"string","minLength":1,"maxLength":500}},"required":[field],"additionalProperties":false})));
    }
    out.push(ToolDefinition::new("verify_link", "Check fetched recording and work against catalog identity and explicit source relationships. Cannot write data.", json!({"type":"object","properties":{"recording_id":{"type":"string"},"work_id":{"type":"string"}},"required":["recording_id","work_id"],"additionalProperties":false})));
    out
}

fn search_query(kind: &str, args: &Value) -> Result<String> {
    let fields = args
        .as_object()
        .context("search arguments must be an object")?;
    ensure!(!fields.is_empty(), "supply a title or ISRC");
    let mut parts = Vec::new();
    for (key, value) in fields {
        let value = value.as_str().context("search fields must be strings")?;
        ensure!(
            !value.trim().is_empty() && value.len() <= 200,
            "invalid search field"
        );
        let field=match (kind,key.as_str()) {
            ("recording","isrc") => { ensure!(value.len()<=32 && value.bytes().all(|c|c.is_ascii_alphanumeric()),"invalid ISRC"); "isrc" },
            ("recording","title") => "recording",
            ("recording","artist") => "artist",
            ("work","title") => "work",
            _ => anyhow::bail!("unsupported search field; use plain title/artist/isrc parameters, not query syntax"),
        };
        parts.push(format!(
            "{field}:\"{}\"",
            value.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    ensure!(
        fields.contains_key("title") || fields.contains_key("isrc"),
        "supply a title or ISRC"
    );
    Ok(parts.join(" AND "))
}

// Keep API evidence in the verifier, not repeated wholesale in an 8K model context.
fn compact(value: &Value) -> Value {
    match value {
        Value::Array(rows) => Value::Array(rows.iter().take(10).map(compact).collect()),
        Value::Object(fields) => {
            let mut out = serde_json::Map::new();
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "id" | "title"
                        | "name"
                        | "query"
                        | "type"
                        | "target-type"
                        | "direction"
                        | "score"
                        | "count"
                        | "disambiguation"
                        | "isrcs"
                        | "artist-credit"
                        | "artist"
                        | "work"
                        | "recording"
                        | "recordings"
                        | "works"
                        | "relations"
                        | "attributes"
                        | "candidates"
                        | "qid"
                        | "creators"
                        | "kind"
                        | "url"
                        | "entity"
                        | "error"
                        | "verified"
                ) {
                    out.insert(key.clone(), compact(value));
                    if let Some(rows) = value.as_array().filter(|rows| rows.len() > 10) {
                        out.insert(format!("{key}_total"), json!(rows.len()));
                        out.insert(format!("{key}_truncated"), json!(true));
                    }
                }
            }
            Value::Object(out)
        }
        Value::String(s) if s.len() > 1000 => json!({"text_omitted":"field exceeds prompt limit"}),
        _ => value.clone(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    recording_id: Option<String>,
    work_id: Option<String>,
    reason: String,
}

fn parse_decision(text: &str) -> Result<Decision> {
    let text = text.trim();
    // Accept a single, otherwise clean JSON code block. Do not extract an
    // arbitrary JSON fragment from prose, reasoning, or fake tool-call markup.
    let text = if let Some(body) = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
    {
        body.strip_suffix("```")
            .context("unterminated JSON code block")?
            .trim()
    } else {
        text
    };
    serde_json::from_str(text).context("invalid research decision JSON")
}

#[derive(Default, Serialize, Deserialize)]
struct Evidence {
    known_recordings: BTreeSet<String>,
    known_works: BTreeSet<String>,
    recordings: BTreeMap<String, Value>,
    works: BTreeMap<String, Value>,
    requests: BTreeSet<String>,
    trace: Vec<Value>,
}

impl Evidence {
    fn discover(&mut self, kind: &str, value: &Value) {
        if let Some(id) = value["id"].as_str().filter(|s| valid_mbid(s)) {
            if kind == "recording" {
                self.known_recordings.insert(id.into());
            } else {
                self.known_works.insert(id.into());
            }
        }
        for relation in value["relations"].as_array().into_iter().flatten() {
            for target in ["recording", "work"] {
                if relation[target].is_object() {
                    self.discover(target, &relation[target]);
                }
            }
        }
    }

    fn verify(&self, context: &Value, recording_id: &str, work_id: &str) -> Result<WorkEvaluation> {
        let mut recording = self
            .recordings
            .get(recording_id)
            .context("recording must be fetched")?
            .clone();
        let work = self.works.get(work_id).context("work must be fetched")?;
        ensure!(
            recording_matches_catalog(&recording, context),
            "recording does not match catalog ISRC, title and credited artists"
        );
        for relation in recording["relations"].as_array_mut().into_iter().flatten() {
            if relation["work"]["id"] == work_id {
                relation["work"] = work.clone();
            }
        }
        let result = musicbrainz_work_evaluation(&recording);
        ensure!(
            result.identification.work.is_some()
                && result.evidence["selected_source"]["musicbrainz_id"] == work_id,
            "no unique, non-composite performance relationship with fetched writer credits"
        );
        Ok(result)
    }

    async fn call(
        &mut self,
        sources: &dyn Sources,
        context: &Value,
        name: &str,
        args: &Value,
    ) -> Result<Value> {
        let key = progress::request_key(name, args);
        ensure!(key.len() <= 800, "request arguments too large");
        ensure!(
            self.requests.insert(key),
            "duplicate request; use previous result or change query"
        );
        let field = |key: &str| -> Result<&str> {
            let value = args[key].as_str().context("missing string argument")?;
            ensure!(
                !value.trim().is_empty() && value.len() <= 500,
                "invalid argument length"
            );
            Ok(value)
        };
        ensure!(
            matches!(name, "search_recordings" | "search_works")
                || args
                    .as_object()
                    .is_some_and(|a| a.len() == if name == "verify_link" { 2 } else { 1 }),
            "unexpected arguments"
        );
        match name {
            "search_recordings" | "search_works" => {
                let kind = if name == "search_recordings" {
                    "recording"
                } else {
                    "work"
                };
                let query = search_query(kind, args)?;
                let value = sources.search(kind, &query).await?;
                let list = if kind == "recording" {
                    "recordings"
                } else {
                    "works"
                };
                for row in value[list].as_array().into_iter().flatten() {
                    self.discover(kind, row);
                }
                Ok(value)
            }
            "fetch_recording" | "fetch_work" => {
                let kind = if name == "fetch_recording" {
                    "recording"
                } else {
                    "work"
                };
                let id = field("id")?;
                ensure!(
                    valid_mbid(id)
                        && if kind == "recording" {
                            self.known_recordings.contains(id)
                        } else {
                            self.known_works.contains(id)
                        },
                    "ID must come from a source search or fetched relationship"
                );
                let value = sources.fetch(kind, id).await?;
                ensure!(value["id"] == id, "fetched identity changed");
                self.discover(kind, &value);
                if kind == "recording" {
                    self.recordings.insert(id.into(), value.clone());
                } else {
                    self.works.insert(id.into(), value.clone());
                }
                Ok(json!({"url":format!("https://musicbrainz.org/{kind}/{id}"),"entity":value}))
            }
            "search_wikidata_works" => sources.wikidata(field("title")?).await,
            "verify_link" => {
                let result = self.verify(context, field("recording_id")?, field("work_id")?)?;
                Ok(
                    json!({"verified":true,"work":result.identification.work,"evidence":result.evidence}),
                )
            }
            _ => anyhow::bail!("unknown research tool"),
        }
    }
}

pub(super) async fn research(
    provider: &dyn LlmProvider,
    context: Value,
    history: Value,
    timeout_secs: u64,
) -> Result<WorkEvaluation> {
    let sources = ReferenceClient::new()?;
    run(provider, &sources, context, history, timeout_secs).await
}

async fn run(
    provider: &dyn LlmProvider,
    sources: &dyn Sources,
    context: Value,
    history: Value,
    timeout_secs: u64,
) -> Result<WorkEvaluation> {
    let started = std::time::Instant::now();
    let track_id = context["track"]["id"].as_str().unwrap_or("unknown");
    let mut evidence = Evidence::restore(&context, &history);
    tracing::info!(
        track_id,
        provider = provider.name(),
        model = provider.model(),
        remembered_requests = evidence.requests.len(),
        "Work research started"
    );
    let definitions = tools();
    let mut remaining = 10usize;
    let mut rounds = Vec::new();
    let mut verified = evidence.verified(&context);
    let research = async {
        for round in 0..progress::ROUNDS {
            if verified.is_some() || remaining == 0 {
                break;
            }
            tracing::info!(
                track_id,
                round = round + 1,
                tool_calls_left = remaining,
                "Work research round started"
            );
            let before = remaining;
            // Advance a known search/fetch step even if the previous model call
            // repeated itself, abstained prematurely, or failed to produce JSON.
            if let Some((name, args)) = evidence.next_action(&context) {
                remaining -= 1;
                evidence
                    .execute(sources, &context, name, &args, round + 1, "application")
                    .await;
            }
            verified = evidence.verified(&context);
            if verified.is_some() || remaining == 0 {
                rounds.push(json!({"round":round+1,"new_actions":before-remaining,"outcome":if verified.is_some(){"verified"}else{"tool_budget_exhausted"}}));
                break;
            }
            let (messages, _) = state::messages(&evidence, &context, &history, round, remaining);
            evidence
                .trace
                .push(json!({"prompt_turn":round+1,"phase":"research","messages":messages}));
            let completion_started = std::time::Instant::now();
            let response = provider
                .complete(
                    &messages,
                    Some(&definitions),
                    &CompletionOptions {
                        temperature: 0.0,
                        max_tokens: Some(4096),
                        timeout: Duration::from_secs(timeout_secs.min(120)),
                    },
                )
                .await;
            let step: Result<()> = async {
                let response = response?;
                tracing::info!(track_id,round=round+1,finish_reason=?response.finish_reason,elapsed_ms=completion_started.elapsed().as_millis() as u64,total_tokens=response.usage.map(|u|u.total_tokens),"Work research model response received");
                evidence.trace.push(json!({"turn":round+1,"response":response.message,"finish_reason":format!("{:?}",response.finish_reason),"elapsed_ms":completion_started.elapsed().as_millis() as u64,"usage":response.usage.map(|u|json!({"prompt_tokens":u.prompt_tokens,"completion_tokens":u.completion_tokens,"total_tokens":u.total_tokens}))}));
                if let Some(calls)=response.message.tool_calls.as_ref().filter(|calls|!calls.is_empty()) {
                    ensure!(matches!(response.finish_reason,FinishReason::ToolCalls|FinishReason::Stop),"partial tool response rejected");
                    ensure!(calls.len()<=remaining,"research tool-call budget exceeded");
                    for call in calls {
                        if evidence.requests.contains(&progress::request_key(&call.name,&call.arguments)) {
                            tracing::info!(track_id,round=round+1,tool=call.name,"Skipped repeated work research action");
                            evidence.trace.push(json!({"round":round+1,"round_error":"duplicate action skipped; advance to an untried action","rejected_tool":call.name}));
                            continue;
                        }
                        remaining-=1;
                        evidence.execute(sources,&context,&call.name,&call.arguments,round+1,"model").await;
                    }
                } else {
                    ensure!(response.finish_reason==FinishReason::Stop,"incomplete research answer; no link saved");
                    let decision=parse_decision(&response.message.content)?;
                    ensure!(!decision.reason.trim().is_empty() && decision.reason.len()<=4000,"invalid research reason");
                    // A model conclusion cannot terminate useful source research
                    // or authorize a link. Source validation decides independently.
                    if let (Some(recording),Some(work))=(&decision.recording_id,&decision.work_id) {
                        evidence.verify(&context,recording,work)?;
                    }
                    evidence.trace.push(json!({"round":round+1,"round_error":"model supplied a conclusion; continue if untried source actions remain"}));
                }
                Ok(())
            }.await;
            if let Err(error) = step {
                tracing::warn!(track_id,round=round+1,error=%error,"Work research model step failed");
                evidence
                    .trace
                    .push(json!({"round":round+1,"round_error":format!("{error:#}")}));
            }
            verified = evidence.verified(&context);
            rounds.push(json!({"round":round+1,"new_actions":before-remaining,"outcome":if verified.is_some(){"verified"}else{"unresolved"}}));
        }
    };
    if crate::execution::time::timeout(Duration::from_secs(240), research)
        .await
        .is_err()
    {
        evidence
            .trace
            .push(json!({"round_error":"research exceeded its 240-second budget"}));
    }
    let conclusion = format!("No independently verified work link after {} research rounds; prior evidence and attempted searches retained",rounds.len());
    tracing::info!(
        track_id,
        verified = verified.is_some(),
        rounds = rounds.len(),
        tool_calls = 10 - remaining,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "Work research finished"
    );
    let checkpoint = evidence.checkpoint(&context);
    let mut result = verified.unwrap_or_else(|| WorkEvaluation {
        identification: Identification {
            work: None,
            reason: conclusion,
            wikidata_id: None,
        },
        validation_error: None,
        evidence: json!({"prompt_version":"work-research-v3-rounds","context":context}),
    });
    result.evidence["research"] = json!({"prompt_version":"work-research-v3-rounds","provider":provider.name(),"model":provider.model(),"rounds":rounds,"checkpoint":checkpoint,"trace":evidence.trace});
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{llm::ToolCall, CompletionResponse, LlmError};
    use std::sync::Mutex;
    const RECORDING: &str = "00000000-0000-0000-0000-000000000001";
    const WORK: &str = "00000000-0000-0000-0000-000000000002";
    fn context() -> Value {
        json!({"track":{"name":"Song","external_id_isrc":"USAAA1200001"},"artists":[{"artist":{"name":"Singer"}}]})
    }
    fn recording() -> Value {
        json!({"id":RECORDING,"title":"Song","isrcs":["USAAA1200001"],"artist-credit":[{"artist":{"name":"Singer"}}],"relations":[{"type":"performance","target-type":"work","work":{"id":WORK,"title":"Song"}}]})
    }
    fn work() -> Value {
        json!({"id":WORK,"title":"Song","type":"Song","relations":[{"type":"composer","artist":{"name":"Writer"}}]})
    }
    struct Fixture;
    #[async_trait::async_trait]
    impl Sources for Fixture {
        async fn search(&self, _: &str, _: &str) -> Result<Value> {
            Ok(json!({"recordings":[recording()]}))
        }
        async fn fetch(&self, kind: &str, _: &str) -> Result<Value> {
            Ok(if kind == "recording" {
                recording()
            } else {
                work()
            })
        }
        async fn wikidata(&self, _: &str) -> Result<Value> {
            Ok(json!({"candidates":[]}))
        }
    }
    struct EmptySources;
    #[async_trait::async_trait]
    impl Sources for EmptySources {
        async fn search(&self, _: &str, _: &str) -> Result<Value> {
            Ok(json!({"recordings":[],"works":[],"count":0}))
        }
        async fn fetch(&self, _: &str, _: &str) -> Result<Value> {
            anyhow::bail!("no discovered records")
        }
        async fn wikidata(&self, _: &str) -> Result<Value> {
            Ok(json!({"candidates":[]}))
        }
    }
    struct Model(Mutex<std::collections::VecDeque<CompletionResponse>>);
    #[async_trait::async_trait]
    impl LlmProvider for Model {
        fn name(&self) -> &str {
            "fixture"
        }
        fn model(&self) -> &str {
            "research"
        }
        async fn health_check(&self) -> std::result::Result<(), LlmError> {
            Ok(())
        }
        async fn complete(
            &self,
            messages: &[Message],
            tools: Option<&[ToolDefinition]>,
            _: &CompletionOptions,
        ) -> std::result::Result<CompletionResponse, LlmError> {
            assert_eq!(
                messages.len(),
                2,
                "each step gets a fresh system prompt and state"
            );
            assert_eq!(messages[0].role, crate::agent::llm::MessageRole::System);
            assert_eq!(messages[1].role, crate::agent::llm::MessageRole::User);
            assert!(!messages
                .iter()
                .any(|m| m.content.contains("MODEL_ONLY_CLAIM")));
            let state: Value = serde_json::from_str(&messages[1].content).unwrap();
            assert_eq!(tools.is_none(), state["phase"] == "decide");
            if let Some(tools) = tools {
                assert_eq!(tools.len(), 6);
            }
            Ok(self
                .0
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected model call"))
        }
    }
    fn response(message: Message, finish_reason: FinishReason) -> CompletionResponse {
        CompletionResponse {
            message,
            finish_reason,
            usage: None,
        }
    }
    fn call(name: &str, arguments: Value) -> CompletionResponse {
        response(
            Message::assistant_with_tools(
                "MODEL_ONLY_CLAIM",
                vec![ToolCall {
                    id: name.into(),
                    name: name.into(),
                    arguments,
                }],
            ),
            FinishReason::ToolCalls,
        )
    }
    fn final_answer() -> CompletionResponse {
        response(
            Message::assistant(
                json!({"recording_id":RECORDING,"work_id":WORK,"reason":"claims a match"})
                    .to_string(),
            ),
            FinishReason::Stop,
        )
    }

    #[simple_server::test(host_runtime = true)]
    async fn research_links_only_fetched_corroborated_recordings_and_credits() {
        let model = Model(Mutex::new(
            vec![
                call("search_recordings", json!({"isrc":"USAAA1200001"})),
                call("fetch_recording", json!({"id":RECORDING})),
                call("fetch_work", json!({"id":WORK})),
                final_answer(),
            ]
            .into(),
        ));
        let result = run(&model, &Fixture, context(), json!([]), 10)
            .await
            .unwrap();
        assert_eq!(result.identification.work.unwrap().creators, vec!["Writer"]);
        assert_eq!(result.evidence["selected_source"]["musicbrainz_id"], WORK);
        assert!(
            result.evidence["research"]["trace"]
                .as_array()
                .unwrap()
                .len()
                >= 9
        );
    }

    #[simple_server::test(host_runtime = true)]
    async fn research_model_claim_cannot_create_link_without_sources() {
        let model = Model(Mutex::new((0..5).map(|_| final_answer()).collect()));
        let result = run(&model, &EmptySources, context(), json!([]), 10)
            .await
            .unwrap();
        assert!(result.identification.work.is_none());
        assert!(result.evidence["research"]["trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["round_error"]
                .as_str()
                .is_some_and(|s| s.contains("recording must be fetched"))));
    }

    #[simple_server::test(host_runtime = true)]
    async fn research_rejects_invented_ids_unknown_tools_and_duplicate_calls() {
        let mut evidence = Evidence::default();
        assert!(evidence
            .call(
                &Fixture,
                &context(),
                "fetch_recording",
                &json!({"id":RECORDING})
            )
            .await
            .is_err());
        assert!(evidence
            .call(
                &Fixture,
                &context(),
                "fetch_url",
                &json!({"url":"http://localhost/"})
            )
            .await
            .is_err());
        assert!(evidence
            .call(
                &Fixture,
                &context(),
                "search_recordings",
                &json!({"title":"Song"})
            )
            .await
            .is_ok());
        assert!(evidence
            .call(
                &Fixture,
                &context(),
                "search_recordings",
                &json!({"title":"Song"})
            )
            .await
            .is_err());
    }

    #[test]
    fn research_rejects_wrong_recording_medleys_and_missing_credits() {
        let mut evidence = Evidence::default();
        evidence.recordings.insert(RECORDING.into(), recording());
        evidence.works.insert(WORK.into(), work());
        let mut wrong = context();
        wrong["track"]["external_id_isrc"] = json!("OTHER");
        assert!(evidence.verify(&wrong, RECORDING, WORK).is_err());
        wrong = context();
        wrong["track"]["name"] = json!("Song - Live");
        assert!(evidence.verify(&wrong, RECORDING, WORK).is_err());
        evidence.recordings.get_mut(RECORDING).unwrap()["relations"][0]["attributes"] =
            json!(["medley"]);
        assert!(evidence.verify(&context(), RECORDING, WORK).is_err());
        evidence.recordings.insert(RECORDING.into(), recording());
        evidence.works.get_mut(WORK).unwrap()["relations"] = json!([]);
        assert!(evidence.verify(&context(), RECORDING, WORK).is_err());
    }

    #[test]
    fn research_real_remaster_links_only_with_complete_unambiguous_source_evidence() {
        let fixture: Value =
            serde_json::from_str(include_str!("fixtures/two_of_a_mind_remaster.json")).unwrap();
        let recording_id = fixture["recording"]["id"].as_str().unwrap();
        let work_id = fixture["work"]["id"].as_str().unwrap();
        let mut evidence = Evidence::default();
        evidence
            .recordings
            .insert(recording_id.into(), fixture["recording"].clone());
        evidence
            .works
            .insert(work_id.into(), fixture["work"].clone());
        let verified = evidence
            .verify(&fixture["context"], recording_id, work_id)
            .unwrap();
        assert_eq!(
            verified.identification.work.unwrap().creators,
            vec!["Paul Desmond"]
        );
        assert_eq!(
            verified.evidence["selected_source"]["musicbrainz_id"],
            work_id
        );
        for title in [
            "Two of a Mind - Live",
            "Two of a Mind - Remix - 2003 Remastered",
            "Two of a Mind (Part II)",
        ] {
            let mut context = fixture["context"].clone();
            context["track"]["name"] = json!(title);
            assert!(
                evidence.verify(&context, recording_id, work_id).is_err(),
                "{title}"
            );
        }
        evidence.works.get_mut(work_id).unwrap()["relations"] = json!([]);
        assert!(
            evidence
                .verify(&fixture["context"], recording_id, work_id)
                .is_err(),
            "no fetched writer credits"
        );
        evidence
            .works
            .insert(work_id.into(), fixture["work"].clone());
        for attribute in ["medley", "partial"] {
            let mut recording = fixture["recording"].clone();
            let relation = recording["relations"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["type"] == "performance")
                .unwrap();
            relation["attributes"] = json!([attribute]);
            evidence.recordings.insert(recording_id.into(), recording);
            assert!(
                evidence
                    .verify(&fixture["context"], recording_id, work_id)
                    .is_err(),
                "{attribute}"
            );
        }
        let mut recording = fixture["recording"].clone();
        recording["relations"].as_array_mut().unwrap().push(json!({"type":"performance","target-type":"work","work":{"id":"00000000-0000-0000-0000-000000000099","title":"Another composition"}}));
        evidence.recordings.insert(recording_id.into(), recording);
        assert!(
            evidence
                .verify(&fixture["context"], recording_id, work_id)
                .is_err(),
            "multiple performed works"
        );
    }

    #[test]
    fn research_compacts_unbounded_api_fields() {
        let value = json!({"recordings":[{"id":RECORDING,"title":"Song","releases":vec![json!({"huge":"x".repeat(10000)})],"relations":vec![json!({"type":"performance","work":{"id":WORK,"title":"Song"}});20]}]});
        let small = compact(&value);
        assert_eq!(
            small["recordings"][0]["relations"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        assert_eq!(small["recordings"][0]["relations_truncated"], true);
        assert!(small["recordings"][0].get("releases").is_none());
    }

    #[test]
    fn research_search_fields_are_explicit_and_lucene_escaped() {
        assert_eq!(
            search_query("recording", &json!({"isrc":"USAAA1200001"})).unwrap(),
            "isrc:\"USAAA1200001\""
        );
        assert_eq!(
            search_query("work", &json!({"title":"A \"Song\""})).unwrap(),
            "work:\"A \\\"Song\\\"\""
        );
        assert!(search_query("work", &json!({"query":"title:Song"})).is_err());
        assert!(search_query("recording", &json!({"artist":"Singer"})).is_err());
        assert!(search_query("recording", &json!({"isrc":"* OR *"})).is_err());
    }

    #[test]
    fn research_decision_accepts_clean_json_fences_but_not_mixed_prose_or_tool_markup() {
        let decision = r#"{"recording_id":null,"work_id":null,"reason":"No evidence"}"#;
        for text in [
            decision.to_owned(),
            format!("```json\n{decision}\n```"),
            format!("```\n{decision}\n```"),
        ] {
            assert!(parse_decision(&text).unwrap().work_id.is_none());
        }
        for text in [
            format!("Some explanation\n{decision}"),
            format!("<tool_call>{decision}</tool_call>"),
            format!("```json\n{decision}"),
            format!("{decision}\n{decision}"),
        ] {
            assert!(parse_decision(&text).is_err());
        }
    }

    #[test]
    fn research_state_preserves_evidence_without_model_claims_and_separates_decision() {
        let mut evidence = Evidence::default();
        let mut r = recording();
        let performance = r["relations"][0].clone();
        r["relations"] = json!(vec![
            json!({"type":"instrument","target-type":"artist","artist":{"name":"Player"}});
            20
        ]);
        r["relations"].as_array_mut().unwrap().push(performance);
        evidence.recordings.insert(RECORDING.into(), r);
        evidence.works.insert(WORK.into(), work());
        evidence
            .trace
            .push(json!({"response":{"content":"MODEL_ONLY_CLAIM"}}));
        evidence.trace.push(json!({"tool":"search_recordings","arguments":{"isrc":"USAAA1200001"},"result":{"recordings":[recording()]}}));
        let (messages, deciding) = state::messages(&evidence, &context(), &json!([]), 3, 7);
        assert!(deciding, "verified evidence ends research early");
        let snapshot: Value = serde_json::from_str(&messages[1].content).unwrap();
        assert_eq!(snapshot["verified_pairs"][0]["work_id"], WORK);
        assert_eq!(
            snapshot["fetched_recordings"]["items"][0]["performance_links"]["items"][0]["work_id"],
            WORK
        );
        assert!(!messages
            .iter()
            .any(|m| m.content.contains("MODEL_ONLY_CLAIM")));
        assert!(!messages
            .iter()
            .any(|m| m.content.contains("search_recordings")));
        assert!(snapshot.get("budget").is_none());
        assert!(snapshot.get("candidates").is_none());
    }

    #[test]
    fn research_state_bounds_prompt_and_keeps_empty_results_distinct_from_errors() {
        let mut evidence = Evidence::default();
        evidence.trace.push(json!({"tool":"search_recordings","arguments":{"isrc":"USAAA1200001"},"result":{"recordings":[],"count":0}}));
        evidence.trace.push(json!({"tool":"search_works","arguments":{"title":"Song"},"result":{"error":"source unavailable"}}));
        for n in 0..10 {
            let id = format!("00000000-0000-0000-0000-{n:012}");
            let mut r = recording();
            r["id"] = json!(id);
            r["title"] = json!("x".repeat(5000));
            evidence.recordings.insert(id, r);
        }
        let (messages, deciding) = state::messages(&evidence, &context(), &json!([]), 2, 8);
        assert!(!deciding);
        assert!(messages[1].content.len() <= 12000);
        let snapshot: Value = serde_json::from_str(&messages[1].content).unwrap();
        assert_eq!(
            snapshot["attempted_requests"]["items"][0]["returned_candidates"],
            0
        );
        assert!(snapshot["attempted_requests"]["items"][1]["returned_candidates"].is_null());
        assert_eq!(
            snapshot["attempted_requests"]["items"][1]["error"],
            "source unavailable"
        );
        assert!(snapshot["fetched_recordings"]["omitted"].as_u64().unwrap() > 0);
    }

    #[simple_server::test(host_runtime = true)]
    async fn research_stops_after_five_rounds_and_carries_partial_output_failure_forward() {
        let model = Model(Mutex::new(
            (0..5)
                .map(|_| {
                    let mut partial = final_answer();
                    partial.finish_reason = FinishReason::MaxTokens;
                    partial
                })
                .collect(),
        ));
        let result = run(&model, &EmptySources, context(), json!([]), 10)
            .await
            .unwrap();
        assert!(result.identification.work.is_none());
        assert_eq!(
            result.evidence["research"]["rounds"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        let trace = result.evidence["research"]["trace"].as_array().unwrap();
        assert_eq!(
            trace.iter().filter(|t| t["response"].is_object()).count(),
            5
        );
        let prompt = &trace
            .iter()
            .filter(|t| t["prompt_turn"].is_number())
            .nth(1)
            .unwrap()["messages"][1]["content"];
        assert!(prompt
            .as_str()
            .unwrap()
            .contains("incomplete research answer"));
        assert!(trace.iter().filter(|t| t["tool"].is_string()).count() <= 10);
    }

    #[simple_server::test(host_runtime = true)]
    async fn research_repeated_queries_advance_and_resume_across_interventions() {
        let model = Model(Mutex::new(
            (0..5)
                .map(|_| call("search_recordings", json!({"isrc":"usaaa1200001"})))
                .collect(),
        ));
        let first = run(&model, &EmptySources, context(), json!([]), 10)
            .await
            .unwrap();
        let trace = first.evidence["research"]["trace"].as_array().unwrap();
        let searches: Vec<_> = trace
            .iter()
            .filter(|t| t["tool"] == "search_recordings")
            .collect();
        assert_eq!(
            searches.len(),
            2,
            "ISRC then title/artist, never repeated source requests"
        );
        assert_eq!(
            searches[1]["arguments"],
            json!({"title":"Song","artist":"Singer"})
        );
        let history = json!([{"diagnostics":attempt_diagnostics(&first.evidence)}]);
        assert!(history.to_string().len() < 65536);
        let model = Model(Mutex::new(
            (0..5)
                .map(|_| {
                    call(
                        "search_recordings",
                        json!({"title":"Alternative title","artist":"Singer"}),
                    )
                })
                .collect(),
        ));
        let second = run(&model, &EmptySources, context(), history, 10)
            .await
            .unwrap();
        let trace = second.evidence["research"]["trace"].as_array().unwrap();
        let resumed = trace
            .iter()
            .position(|t| t["memory_restored"] == true)
            .unwrap();
        let new_searches: Vec<_> = trace[resumed..]
            .iter()
            .filter(|t| t["tool"] == "search_recordings")
            .collect();
        assert_eq!(new_searches.len(), 1);
        assert_eq!(new_searches[0]["arguments"]["title"], "Alternative title");
        let first_prompt = &trace[resumed..]
            .iter()
            .find(|t| t["prompt_turn"].is_number())
            .unwrap()["messages"][1]["content"];
        assert!(first_prompt.as_str().unwrap().contains("USAAA1200001"));
        assert!(first_prompt
            .as_str()
            .unwrap()
            .contains("returned_candidates\":0"));
    }

    #[test]
    fn research_checkpoint_preserves_complete_evidence_and_invalidates_changed_identity() {
        let mut evidence = Evidence::default();
        evidence.discover("recording", &recording());
        evidence.recordings.insert(RECORDING.into(), recording());
        evidence.works.insert(WORK.into(), work());
        let history =
            json!([{"diagnostics":{"research":{"checkpoint":evidence.checkpoint(&context())}}}]);
        let restored = Evidence::restore(&context(), &history);
        assert!(restored.verify(&context(), RECORDING, WORK).is_ok());
        let mut changed = context();
        changed["track"]["external_id_isrc"] = json!("CHANGED");
        assert!(Evidence::restore(&changed, &history).recordings.is_empty());
        evidence.recordings.get_mut(RECORDING).unwrap()["huge_unrelated_field"] =
            json!("x".repeat(100000));
        evidence.requests.insert(progress::request_key(
            "fetch_recording",
            &json!({"id":RECORDING}),
        ));
        let checkpoint = evidence.checkpoint(&context());
        assert!(checkpoint.to_string().len() <= 48000);
        let restored = Evidence::restore(
            &context(),
            &json!([{"diagnostics":{"research":{"checkpoint":checkpoint}}}]),
        );
        assert!(
            !restored.recordings.contains_key(RECORDING),
            "whole oversized document evicted, never partially validated"
        );
        assert!(!restored.requests.contains(&progress::request_key(
            "fetch_recording",
            &json!({"id":RECORDING})
        )));
    }

    #[simple_server::test(host_runtime = true)]
    async fn research_memory_distinguishes_failed_source_requests_from_empty_successes() {
        let mut evidence = Evidence::default();
        evidence
            .execute(
                &EmptySources,
                &context(),
                "search_recordings",
                &json!({"isrc":"USAAA1200001"}),
                1,
                "application",
            )
            .await;
        evidence
            .execute(
                &EmptySources,
                &context(),
                "fetch_recording",
                &json!({"id":RECORDING}),
                1,
                "model",
            )
            .await;
        let restored = Evidence::restore(
            &context(),
            &json!([{"diagnostics":{"research":{"checkpoint":evidence.checkpoint(&context())}}}]),
        );
        assert!(restored.requests.contains(&progress::request_key(
            "search_recordings",
            &json!({"isrc":"USAAA1200001"})
        )));
        assert!(!restored.requests.contains(&progress::request_key(
            "fetch_recording",
            &json!({"id":RECORDING})
        )));
    }

    /// Replay complete saved source documents through the current validator.
    /// No model, reference API, or production database is used.
    #[test]
    #[ignore = "requires exported contexts and saved research checkpoints"]
    fn work_research_saved_corpus() {
        let input: Value = serde_json::from_slice(
            &std::fs::read(std::env::var("PEZZOTTIFY_RESEARCH_INPUT").unwrap()).unwrap(),
        )
        .unwrap();
        let saved = std::path::PathBuf::from(std::env::var("PEZZOTTIFY_RESEARCH_SAVED").unwrap());
        let mut results = Vec::new();
        for (index, item) in input["items"].as_array().unwrap().iter().enumerate() {
            let previous: Value = serde_json::from_slice(
                &std::fs::read(saved.join(format!("{:02}.json", index + 1))).unwrap(),
            )
            .unwrap();
            let history = json!([{"diagnostics":{"research":{"checkpoint":previous["evaluation"]["evidence"]["research"]["checkpoint"]}}}]);
            let evidence = Evidence::restore(&item["context"], &history);
            let evaluated = evidence.verified(&item["context"]);
            println!(
                "item {}: {} accepted={}",
                index + 1,
                item["context"]["track"]["name"],
                evaluated.is_some()
            );
            results.push(json!({"track_id":item["track_id"],"title":item["context"]["track"]["name"],"complete_recordings_replayed":evidence.recordings.len(),"complete_works_replayed":evidence.works.len(),"evaluation":evaluated}));
        }
        std::fs::write(
            std::env::var("PEZZOTTIFY_RESEARCH_OUTPUT").unwrap(),
            serde_json::to_vec_pretty(&results).unwrap(),
        )
        .unwrap();
    }

    /// Opt-in production-model evaluation, using exported contexts only; no store is opened.
    #[simple_server::test(host_runtime = true)]
    #[ignore = "live model and reference APIs; requires PEZZOTTIFY_RESEARCH_INPUT and PEZZOTTIFY_RESEARCH_OUTPUT"]
    async fn work_research_live_corpus() {
        let input: Value = serde_json::from_slice(
            &std::fs::read(std::env::var("PEZZOTTIFY_RESEARCH_INPUT").unwrap()).unwrap(),
        )
        .unwrap();
        let out = std::path::PathBuf::from(std::env::var("PEZZOTTIFY_RESEARCH_OUTPUT").unwrap());
        std::fs::create_dir_all(&out).unwrap();
        let provider = crate::agent::llm::OpenAIProvider::with_key_command(
            input["base_url"].as_str().unwrap(),
            input["model"].as_str().unwrap(),
            input["api_key_command"].as_str().unwrap().into(),
        )
        .with_reasoning_effort(input["reasoning_effort"].as_str().map(str::to_owned));
        for (index, item) in input["items"].as_array().unwrap().iter().enumerate() {
            let started = std::time::Instant::now();
            let result = if input["sources_only"] == true {
                async {
                    let source = ReferenceClient::new()?;
                    let recording = source
                        .music_entity("track", &item["context"], None)
                        .await?
                        .context("ordinary source lookup did not identify a unique recording")?;
                    Ok::<_, anyhow::Error>(musicbrainz_work_evaluation(&recording))
                }
                .await
            } else {
                research(
                    &provider,
                    item["context"].clone(),
                    item["history"].clone(),
                    120,
                )
                .await
            };
            let report = match result {
                Ok(evaluation) => json!({"track_id":item["track_id"],"evaluation":evaluation}),
                Err(error) => json!({"track_id":item["track_id"],"error":format!("{error:#}")}),
            };
            std::fs::write(
                out.join(format!("{:02}.json", index + 1)),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            println!(
                "item {}: {} elapsed={}s result={}",
                index + 1,
                item["context"]["track"]["name"],
                started.elapsed().as_secs(),
                report["evaluation"]["identification"]
                    .as_object()
                    .map(|_| report["evaluation"]["identification"].to_string())
                    .unwrap_or_else(|| report["error"].to_string())
            );
        }
    }
}
