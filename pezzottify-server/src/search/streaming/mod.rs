//! Streaming structured search module.
//!
//! Provides a streaming search endpoint that returns structured, enriched results
//! progressively via Server-Sent Events (SSE).

mod enrichment;
mod pipeline;
mod sections;
mod target_identifier;

pub use enrichment::*;
pub use pipeline::StreamingSearchPipeline;
pub use sections::*;
pub use target_identifier::*;
