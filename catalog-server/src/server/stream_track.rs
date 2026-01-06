//! Audio streaming functionality

use super::{
    session::Session,
    state::{GuardedCatalogStore, ServerState},
};
use axum::{
    body::Body,
    extract::{FromRequestParts, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use tokio::{
    fs::File,
    io::{AsyncSeekExt, BufReader, SeekFrom},
};
use tokio_util::io::ReaderStream;
use tracing::debug;

const HEADER_BYTE_RANGE: &str = "Range";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    start_inclusive: Option<u64>,
    end_inclusive: Option<u64>,
}

impl ByteRange {
    pub fn new(start_inclusive: Option<u64>, end_inclusive: Option<u64>) -> ByteRange {
        ByteRange {
            start_inclusive,
            end_inclusive,
        }
    }

    fn parse<S: AsRef<str>>(s: S) -> Option<ByteRange> {
        let v = s.as_ref();
        if !v.starts_with("bytes=") {
            return None;
        }

        let v = &v[6..];
        let parts: Vec<&str> = v.split('-').collect();
        if parts.len() != 2 {
            return None;
        }

        Some(ByteRange {
            start_inclusive: parts[0].parse::<u64>().ok(),
            end_inclusive: parts[1].parse::<u64>().ok(),
        })
    }
}

pub struct ByteRangeExtractionError {}

impl IntoResponse for ByteRangeExtractionError {
    fn into_response(self) -> Response {
        StatusCode::BAD_REQUEST.into_response()
    }
}

impl FromRequestParts<ServerState> for Option<ByteRange> {
    type Rejection = ByteRangeExtractionError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        Ok(parts
            .headers
            .get(HEADER_BYTE_RANGE)
            .map(|x| x.to_str())
            .map(|x| x.ok())
            .and_then(|x| x.and_then(ByteRange::parse)))
    }
}

pub async fn stream_track(
    _session: Session,
    byte_range: Option<ByteRange>,
    State(catalog_store): State<GuardedCatalogStore>,
    Path(id): Path<String>,
) -> Response {
    // Get track metadata
    let track = match catalog_store.get_track(&id) {
        Ok(Some(track)) => track,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    debug!("Streaming track: {}", track.name);

    // Get audio file path - returns None if audio not available
    let path = match catalog_store.get_track_audio_path(&id) {
        None => {
            debug!("Track {} audio not available", track.name);
            return StatusCode::NOT_FOUND.into_response();
        }
        Some(x) => x,
    };
    debug!("Streaming track from path {}", path.display());

    let mut file = match File::open(&path).await {
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Ok(x) => x,
    };

    let mut start_served = 0;
    if let Some(start) = byte_range.and_then(|x| x.start_inclusive) {
        if file.seek(SeekFrom::Start(start)).await.is_err() {
            return StatusCode::BAD_REQUEST.into_response();
        }
        start_served = start;
    }

    let file_length = match file.metadata().await {
        Ok(x) => x.len(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let chunk_size = match byte_range {
        None => file_length,
        Some(ByteRange {
            start_inclusive: None,
            end_inclusive: None,
        }) => file_length,
        Some(ByteRange {
            start_inclusive: None,
            end_inclusive: Some(end),
        }) => end,
        Some(ByteRange {
            start_inclusive: Some(start),
            end_inclusive: None,
        }) => file_length - start,
        Some(ByteRange {
            start_inclusive: Some(start),
            end_inclusive: Some(end),
        }) => end - start + 1,
    };
    let status_code = match byte_range {
        None
        | Some(ByteRange {
            start_inclusive: None,
            end_inclusive: None,
        }) => StatusCode::OK,
        _ => StatusCode::PARTIAL_CONTENT,
    };

    let file_reader = BufReader::with_capacity(4096 * 16, file);
    let stream = ReaderStream::with_capacity(file_reader, 4096 * 16);

    let body = Body::from_stream(stream);

    Response::builder()
        .status(status_code)
        .header("Content-Type", "audio/ogg")
        .header("Accept-Ranges", "bytes")
        .header(
            "Content-Range",
            format!(
                "bytes {}-{}/{}",
                start_served,
                start_served + chunk_size - 1,
                file_length
            ),
        )
        .header("Content-length", chunk_size)
        .body(body)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::ByteRange;

    fn assert_byte_range(s: &str, a: Option<u64>, b: Option<u64>) {
        assert_eq!(ByteRange::parse(s), Some(ByteRange::new(a, b)));
    }

    fn assert_no_byte_range(s: &str) {
        assert_eq!(ByteRange::parse(s), None);
    }

    #[test]
    fn parses_byte_range() {
        assert_no_byte_range("asd");
        assert_no_byte_range("bytes=");
        assert_byte_range("bytes=-", None, None);
        assert_byte_range("bytes=11-", Some(11), None);
        assert_byte_range("bytes=-111", None, Some(111));
        assert_byte_range("bytes=11-111", Some(11), Some(111));
    }
}
