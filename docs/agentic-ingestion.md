# Agentic Ingestion Feature - Implementation Plan

## Feature Summary

**Agentic Ingestion** is an LLM-powered feature that intelligently processes user-uploaded audio files and adds them to the Pezzottify music catalog. Instead of requiring manual metadata entry and file placement, an AI agent analyzes uploaded files, matches them to the correct catalog entries, handles format conversion, and manages the entire ingestion pipeline automatically.

### Why This Feature?

Currently, adding audio content to the catalog requires manual intervention - someone needs to identify what the audio is, match it to catalog metadata, convert to the right format, and place files in the correct location. This is tedious and error-prone.

With Agentic Ingestion:
- Upload a ZIP of an album → Agent figures out what it is and where each track goes
- Have a pending download request? Upload the files → Agent matches them to what you requested
- Uncertain match? Agent asks for human review instead of guessing wrong

### Target Users

Users with `EditCatalog` permission (typically admins or trusted contributors).

---

## Use Cases

### Use Case 1: Fulfilling a Download Request

**Scenario**: Alice requested the album "OK Computer" by Radiohead through the download manager. The automated download failed, but she found the album elsewhere.

**Flow**:
1. Alice navigates to the Ingestion page in the admin panel
2. She selects "Fulfill Download Request" and picks her pending request for "OK Computer"
3. She uploads `ok_computer.zip` containing 12 MP3 files
4. The agent extracts the ZIP and analyzes each file:
   - Parses filenames: `01 - Airbag.mp3`, `02 - Paranoid Android.mp3`, etc.
   - Probes audio duration with ffprobe
   - Searches the catalog for "OK Computer" tracks
   - Matches each file to the corresponding track by name + duration
5. Agent confidence is 95% → Files are auto-approved
6. Each file is converted to OGG Vorbis 320kbps and placed in the media directory
7. Catalog is updated: all 12 tracks now show as "Available"
8. Alice's download request is marked as fulfilled

### Use Case 2: Spontaneous Upload (Known Album)

**Scenario**: Bob has a FLAC rip of "Abbey Road" and wants to add it to the catalog, which already has the album metadata but no audio.

**Flow**:
1. Bob selects "Spontaneous Upload" in the Ingestion page
2. He drags and drops 17 FLAC files
3. The agent analyzes each file:
   - Extracts artist/album/track from filenames
   - Searches catalog: finds "Abbey Road" by The Beatles
   - Matches tracks by name and duration
4. Agent shows 94% confidence for 16 tracks, 72% for one track (filename was mangled)
5. The 72% match goes to the **Review Queue**
6. Bob sees the agent's reasoning: "File '17 - Her Majesty.flac' (23s) could match 'Her Majesty' (23s) or 'Golden Slumbers' (91s)"
7. Bob confirms it's "Her Majesty"
8. All files are converted and saved
9. "Abbey Road" now shows as fully available

### Use Case 3: Ambiguous Upload (Review Required)

**Scenario**: Carol uploads files with poor naming: `track01.mp3`, `track02.mp3`, etc.

**Flow**:
1. Carol uploads 10 files with no useful metadata in filenames
2. Agent probes durations: 3:45, 4:12, 3:58...
3. Agent searches catalog by duration patterns, finds 3 possible album matches
4. Confidence is low (45%) → All files go to **Review Queue**
5. Agent explains: "These 10 files with durations [3:45, 4:12, ...] could match:
   - 'Thriller' by Michael Jackson (82% duration match)
   - 'Bad' by Michael Jackson (78% duration match)
   - 'Dangerous' by Michael Jackson (71% duration match)"
6. Carol selects "Thriller"
7. Agent re-analyzes with album context, matches tracks by position + duration
8. Files are processed and catalog updated

### Use Case 4: No Match Found

**Scenario**: Dave uploads audio for an album that doesn't exist in the catalog.

**Flow**:
1. Dave uploads `obscure_album.zip`
2. Agent analyzes, searches catalog, finds no matches
3. Agent reports: "Could not find matching album in catalog. Detected: 'Obscure Album' by 'Unknown Artist' (8 tracks). Would you like to create a new catalog entry?"
4. Dave can either:
   - Cancel and add the album metadata first via the catalog editor
   - (Future) Create the album entry inline

---

## User Experience

### Ingestion Dashboard

The admin panel gets a new "Ingestion" section showing:
- **Upload Zone**: Drag-drop area for files/ZIPs
- **Active Jobs**: List of processing jobs with real-time status
- **Review Queue**: Items awaiting human decision
- **History**: Completed/failed ingestion jobs

### Real-Time Feedback

As the agent works, users see:
- Current status: "Analyzing...", "Searching catalog...", "Converting..."
- Step-by-step reasoning: What the agent is thinking
- Confidence scores: How sure the agent is about each match
- WebSocket push updates: No need to refresh

### Review Interface

When human review is needed:
- Agent's question: "Which album is this?"
- Agent's reasoning: Why it's uncertain
- Options: Suggested matches with confidence scores
- Override: Manual track ID entry if agent is wrong

---

## Key Design Decisions

- **LLM on backend** (Rust) - HTTP calls to Ollama (extensible to other providers)
- **Audio conversion** via server-side ffmpeg
- **Manual upload** through web UI
- **Human review queue** for uncertain matches (configurable threshold)
- **Detailed reasoning logs** - step-by-step observability
- **Generic agent module** - reusable for future workflows
- **WebSocket push** for real-time status updates

---

## Architecture

```
catalog-server/src/
├── agent/                      # Generic agent infrastructure
│   ├── mod.rs
│   ├── llm/                    # LLM provider abstraction
│   │   ├── mod.rs
│   │   ├── types.rs            # Message, ToolCall, CompletionResponse
│   │   ├── provider.rs         # LlmProvider trait
│   │   ├── ollama.rs           # Ollama implementation
│   │   └── openai.rs           # OpenAI-compatible implementation
│   ├── tools/                  # Tool registry pattern
│   │   ├── mod.rs
│   │   └── registry.rs         # Tool definitions and execution
│   ├── workflow/               # State machine execution
│   │   ├── mod.rs
│   │   ├── state.rs            # WorkflowState enum
│   │   └── executor.rs         # Step-by-step execution
│   └── reasoning/              # Observability
│       ├── mod.rs
│       └── logger.rs           # ReasoningStep capture
│
├── ingestion/                  # Ingestion-specific logic
│   ├── mod.rs
│   ├── models.rs               # IngestionJob, status enums
│   ├── store.rs                # SqliteIngestionStore
│   ├── schema.rs               # Database schema
│   ├── file_handler.rs         # Upload handling, temp storage
│   ├── converter.rs            # ffmpeg wrapper
│   ├── manager.rs              # IngestionManager facade
│   └── tools.rs                # Ingestion-specific agent tools
│
└── server/
    └── ingestion_routes.rs     # HTTP endpoints
```

---

## 1. LLM Provider Abstraction

### Core Types (`agent/llm/types.rs`)

```rust
pub struct Message {
    pub role: MessageRole,          // System, User, Assistant, Tool
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}

pub struct CompletionResponse {
    pub message: Message,
    pub finish_reason: FinishReason,    // Stop, ToolCalls, MaxTokens
}
```

### Provider Trait (`agent/llm/provider.rs`)

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;

    async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        options: &CompletionOptions,
    ) -> Result<CompletionResponse, LlmError>;

    async fn health_check(&self) -> Result<(), LlmError>;
}
```

### Ollama Implementation (`agent/llm/ollama.rs`)

- POST to `/api/chat` with tool calling support
- Map between Ollama format and common types
- Handle streaming vs non-streaming responses

### OpenAI Implementation (`agent/llm/openai.rs`)

- POST to `/v1/chat/completions` with function calling support
- Compatible with OpenAI, OpenRouter, Together AI, vLLM, and other OpenAI-compatible APIs
- Supports static API keys or dynamic token fetching via shell command
- 10-second timeout on api_key_command execution

---

## 2. Agent Tools

### Tool Registry Pattern (`agent/tools/registry.rs`)

```rust
pub struct AgentToolRegistry {
    tools: HashMap<String, Box<dyn AgentTool>>,
}

#[async_trait]
pub trait AgentTool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<serde_json::Value>;
}
```

### Ingestion Tools (`ingestion/tools.rs`)

| Tool | Description |
|------|-------------|
| `search_catalog` | Search for tracks/albums by name |
| `get_track_info` | Get detailed track metadata |
| `get_album_tracks` | List all tracks in an album |
| `get_pending_requests` | List pending download requests |
| `get_file_metadata` | Get uploaded file info (duration, filename) |
| `set_match` | Commit a track match decision |

---

## 3. Workflow State Machine

### States (`agent/workflow/state.rs`)

```rust
pub enum WorkflowState {
    Started,
    Thinking,                               // Waiting for LLM
    ExecutingTools { tool_calls: Vec<ToolCall> },
    AwaitingReview { question: String, options: Vec<ReviewOption> },
    ReadyToExecute { action: AgentAction },
    Executing,
    Completed { result: WorkflowResult },
    Failed { error: String, recoverable: bool },
}

pub enum AgentAction {
    MatchToTrack { file_id: String, track_id: String, confidence: f32 },
    FulfillDownloadRequest { file_id: String, request_id: String, track_id: String },
    Reject { reason: String },
}
```

### Executor (`agent/workflow/executor.rs`)

```rust
pub struct WorkflowExecutor {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<AgentToolRegistry>,
    reasoning_logger: ReasoningLogger,
    max_iterations: usize,
}

impl WorkflowExecutor {
    /// Run one step, returning new state
    pub async fn step(&self, workflow: &mut Workflow) -> Result<WorkflowState>;

    /// Run until blocked (review needed) or complete
    pub async fn run_until_blocked(&self, workflow: &mut Workflow) -> Result<WorkflowState>;
}
```

---

## 4. Reasoning Logger

### Step Types (`agent/reasoning/logger.rs`)

```rust
pub struct ReasoningStep {
    pub id: String,
    pub timestamp: i64,
    pub step_type: ReasoningStepType,
    pub content: String,
    pub metadata: serde_json::Value,
    pub duration_ms: Option<i64>,
}

pub enum ReasoningStepType {
    Context,        // Initial context
    Thought,        // Agent reasoning
    ToolCall,       // Tool invocation
    ToolResult,     // Tool response
    Decision,       // Match decision
    ReviewQuestion, // Human review needed
    ReviewAnswer,   // Human input received
    Action,         // Final action
    Error,
}
```

---

## 5. Database Schema (`ingestion.db`)

```sql
-- Main job tracking
CREATE TABLE ingestion_jobs (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL,  -- PENDING, PROCESSING, AWAITING_REVIEW, CONVERTING, COMPLETED, FAILED
    user_id TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    temp_file_path TEXT,

    -- ffprobe metadata
    duration_ms INTEGER,
    codec TEXT,
    bitrate INTEGER,

    -- Context
    context_type TEXT,     -- DOWNLOAD_REQUEST or SPONTANEOUS
    context_id TEXT,       -- download_queue_item_id or null

    -- Match result
    matched_track_id TEXT,
    match_confidence REAL,
    match_source TEXT,     -- AGENT or HUMAN_REVIEW

    -- Output
    output_file_path TEXT,
    error_message TEXT,

    -- Timestamps
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,

    -- Workflow state (JSON for resumable workflows)
    workflow_state TEXT
);

-- Step-by-step reasoning log
CREATE TABLE ingestion_reasoning_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL REFERENCES ingestion_jobs(id) ON DELETE CASCADE,
    step_number INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    step_type TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata TEXT,
    duration_ms INTEGER
);

-- Human review queue
CREATE TABLE ingestion_review_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL UNIQUE REFERENCES ingestion_jobs(id) ON DELETE CASCADE,
    question TEXT NOT NULL,
    options TEXT NOT NULL,  -- JSON array
    created_at INTEGER NOT NULL,
    resolved_at INTEGER,
    resolved_by_user_id TEXT,
    selected_option TEXT
);

CREATE INDEX idx_jobs_status ON ingestion_jobs(status);
CREATE INDEX idx_jobs_user ON ingestion_jobs(user_id);
CREATE INDEX idx_reasoning_job ON ingestion_reasoning_log(job_id);
CREATE INDEX idx_review_pending ON ingestion_review_queue(resolved_at) WHERE resolved_at IS NULL;
```

---

## 6. API Routes

All routes require `EditCatalog` permission.

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/ingestion/upload` | Upload file(s), returns job ID(s) |
| GET | `/v1/ingestion/jobs` | List user's jobs |
| GET | `/v1/ingestion/jobs/:id` | Get job detail + reasoning steps |
| DELETE | `/v1/ingestion/jobs/:id` | Cancel pending job |
| GET | `/v1/ingestion/review` | List pending reviews |
| POST | `/v1/ingestion/review/:job_id` | Submit review decision |

### Upload Request

```
POST /v1/ingestion/upload
Content-Type: multipart/form-data

Fields:
- files: File[]
- context_type: "spontaneous" | "download_request"
- context_id?: string (download_queue_item_id)
```

### Job Response

```json
{
  "id": "uuid",
  "status": "PROCESSING",
  "original_filename": "01 - Track Name.mp3",
  "duration_ms": 234000,
  "matched_track_id": "abc123",
  "match_confidence": 0.92,
  "reasoning_steps": [
    { "step_type": "Context", "content": "Analyzing file..." },
    { "step_type": "ToolCall", "content": "search_catalog({...})" },
    { "step_type": "Decision", "content": "Matched to track 'abc123'" }
  ]
}
```

---

## 7. WebSocket Updates

Add to existing WebSocket message types:

```rust
pub enum ServerMessage {
    // ... existing ...

    IngestionUpdate {
        job_id: String,
        status: String,
        latest_step: Option<ReasoningStep>,
        needs_review: bool,
    },
}
```

Broadcast on:
- Job status change
- New reasoning step added
- Review required

---

## 8. Configuration

```toml
[agent]
enabled = true
max_iterations = 20

[agent.llm]
# Provider: "ollama" (default) or "openai" (OpenAI-compatible APIs)
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1:8b"
temperature = 0.3
timeout_secs = 120

# For OpenAI-compatible providers, use one of:
# api_key = "sk-..."                        # Static API key
# api_key_command = "cat /run/secrets/key"  # Dynamic token (10s timeout)

[ingestion]
enabled = true
temp_dir = "{db_dir}/ingestion_uploads"
max_upload_size_mb = 500
auto_approve_threshold = 0.9
ffmpeg_path = "ffmpeg"          # or full path
output_bitrate = "320k"
```

Example with OpenAI:

```toml
[agent.llm]
provider = "openai"
base_url = "https://api.openai.com/v1"
model = "gpt-4o-mini"
api_key = "sk-..."
```

Example with rotating tokens:

```toml
[agent.llm]
provider = "openai"
base_url = "https://api.example.com/v1"
model = "gpt-4o"
api_key_command = "vault kv get -field=token secret/openai"
```

---

## 9. ffmpeg Integration

### Probe (`ingestion/converter.rs`)

```rust
pub async fn probe_file(path: &Path) -> Result<AudioMetadata> {
    // ffprobe -v quiet -print_format json -show_format -show_streams {path}
    // Parse: duration, codec, bitrate, sample_rate
}
```

### Convert

```rust
pub async fn convert_to_ogg(input: &Path, output: &Path, bitrate: &str) -> Result<()> {
    // ffmpeg -i {input} -c:a libvorbis -b:a {bitrate} -y {output}
}
```

---

## 10. Agent System Prompt

```
You are an audio file matching assistant. Your task is to match uploaded
audio files to the correct tracks in the music catalog.

Available tools:
- search_catalog(query): Search for tracks/albums by name
- get_track_info(track_id): Get detailed track metadata
- get_album_tracks(album_id): List all tracks in an album
- get_pending_requests(): List pending download requests
- get_file_metadata(file_id): Get uploaded file info
- set_match(file_id, track_id, confidence, reasoning): Commit a match

Process:
1. Analyze the filename to extract artist, album, track name
2. Search the catalog for potential matches
3. Compare duration (within 5 seconds tolerance)
4. If confident (>90%), set the match directly
5. If uncertain, explain your reasoning and await human review

Always explain your reasoning before making decisions.
```

---

## 11. Files to Create

| Path | Purpose |
|------|---------|
| `catalog-server/src/agent/mod.rs` | Module exports |
| `catalog-server/src/agent/llm/mod.rs` | LLM submodule |
| `catalog-server/src/agent/llm/types.rs` | Common types |
| `catalog-server/src/agent/llm/provider.rs` | LlmProvider trait |
| `catalog-server/src/agent/llm/ollama.rs` | Ollama impl |
| `catalog-server/src/agent/llm/openai.rs` | OpenAI-compatible impl |
| `catalog-server/src/agent/tools/mod.rs` | Tools submodule |
| `catalog-server/src/agent/tools/registry.rs` | Tool registry |
| `catalog-server/src/agent/workflow/mod.rs` | Workflow submodule |
| `catalog-server/src/agent/workflow/state.rs` | State machine |
| `catalog-server/src/agent/workflow/executor.rs` | Executor |
| `catalog-server/src/agent/reasoning/mod.rs` | Reasoning submodule |
| `catalog-server/src/agent/reasoning/logger.rs` | Step logger |
| `catalog-server/src/ingestion/mod.rs` | Ingestion module |
| `catalog-server/src/ingestion/models.rs` | Job models |
| `catalog-server/src/ingestion/store.rs` | SQLite store |
| `catalog-server/src/ingestion/schema.rs` | DB schema |
| `catalog-server/src/ingestion/file_handler.rs` | Upload handling |
| `catalog-server/src/ingestion/converter.rs` | ffmpeg wrapper |
| `catalog-server/src/ingestion/manager.rs` | Manager facade |
| `catalog-server/src/ingestion/tools.rs` | Agent tools |
| `catalog-server/src/server/ingestion_routes.rs` | HTTP routes |
| `web/src/components/admin/IngestionManager.vue` | Admin UI |

## 12. Files to Modify

| Path | Changes |
|------|---------|
| `catalog-server/src/lib.rs` | Add `pub mod agent; pub mod ingestion;` |
| `catalog-server/src/config/mod.rs` | Add AgentConfig, IngestionConfig |
| `catalog-server/src/config/file_config.rs` | Add TOML parsing |
| `catalog-server/src/server/mod.rs` | Mount ingestion routes |
| `catalog-server/src/server/state.rs` | Add IngestionManager |
| `catalog-server/src/server/websocket/messages.rs` | Add IngestionUpdate |
| `catalog-server/src/main.rs` | Initialize agent/ingestion |
| `web/src/views/AdminView.vue` | Add Ingestion section |

---

## 13. Implementation Phases

### Phase 1: Agent Infrastructure
1. Create agent module structure
2. Implement LLM types and traits
3. Implement Ollama provider
4. Add configuration structs
5. Create tool registry pattern

### Phase 2: Ingestion Core
1. Create ingestion module structure
2. Implement database schema and store
3. Implement file handler (upload, temp storage)
4. Implement ffmpeg wrapper (probe, convert)
5. Create ingestion-specific tools

### Phase 3: Workflow Engine
1. Implement workflow state machine
2. Implement reasoning logger
3. Implement workflow executor
4. Create IngestionManager facade

### Phase 4: HTTP API & WebSocket
1. Create ingestion routes
2. Add permission middleware
3. Integrate with server state
4. Add WebSocket messages for updates

### Phase 5: Web UI
1. Create IngestionManager.vue component
2. Add upload form
3. Add job list with status
4. Add reasoning step viewer
5. Add review queue UI

---

## Critical Reference Files

- `catalog-server/src/download_manager/manager.rs` - Facade pattern, async operations
- `catalog-server/src/download_manager/models.rs` - Queue status patterns
- `catalog-server/src/download_manager/queue_store.rs` - SQLite store trait
- `catalog-server/src/mcp/registry.rs` - Tool registration pattern
- `catalog-server/src/config/mod.rs` - Configuration resolution
- `catalog-server/src/server/download_routes.rs` - Route patterns with permissions
- `catalog-server/src/server/websocket/messages.rs` - WebSocket message types
