//! Audio streaming functionality

use super::{
    session::Session,
    state::{GuardedCatalogStore, OptionalOrganicIndexer, ServerState},
};
use axum::{
    body::Body,
    extract::{FromRequestParts, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::path::Path as FilePath;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, BufReader, SeekFrom},
};
use tokio_util::io::ReaderStream;
use tracing::debug;

const STREAM_BUFFER_SIZE: usize = 64 * 1024;

/// A single byte-range specification. Multiple ranges are deliberately unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteRange {
    /// `bytes=start-end`
    Inclusive { start: u64, end: u64 },
    /// `bytes=start-`
    From { start: u64 },
    /// `bytes=-length`
    Suffix { length: u64 },
}

impl ByteRange {
    fn parse(value: &str) -> Result<Self, RangeError> {
        let value = value.trim();
        let (unit, range) = value.split_once('=').ok_or(RangeError::Malformed)?;
        if !unit.eq_ignore_ascii_case("bytes") {
            return Err(RangeError::Malformed);
        }

        // This endpoint supports exactly one range and does not generate multipart responses.
        if range.is_empty() || range.contains(',') {
            return Err(RangeError::Malformed);
        }

        let (start, end) = range.split_once('-').ok_or(RangeError::Malformed)?;
        if end.contains('-') || (start.is_empty() && end.is_empty()) {
            return Err(RangeError::Malformed);
        }

        match (start.is_empty(), end.is_empty()) {
            (false, false) => Ok(Self::Inclusive {
                start: parse_decimal(start)?,
                end: parse_decimal(end)?,
            }),
            (false, true) => Ok(Self::From {
                start: parse_decimal(start)?,
            }),
            (true, false) => {
                let length = parse_decimal(end)?;
                if length == 0 {
                    return Err(RangeError::Unsatisfiable);
                }
                Ok(Self::Suffix { length })
            }
            (true, true) => Err(RangeError::Malformed),
        }
    }

    fn resolve(self, file_length: u64) -> Result<ResolvedRange, RangeError> {
        if file_length == 0 {
            return Err(RangeError::Unsatisfiable);
        }

        let last_file_byte = file_length - 1;
        let (start, end) = match self {
            Self::Inclusive { start, end } => {
                if start > end || start >= file_length {
                    return Err(RangeError::Unsatisfiable);
                }
                (start, end.min(last_file_byte))
            }
            Self::From { start } => {
                if start >= file_length {
                    return Err(RangeError::Unsatisfiable);
                }
                (start, last_file_byte)
            }
            Self::Suffix { length } => {
                if length == 0 {
                    return Err(RangeError::Unsatisfiable);
                }
                (file_length.saturating_sub(length), last_file_byte)
            }
        };

        let length = end
            .checked_sub(start)
            .and_then(|difference| difference.checked_add(1))
            .ok_or(RangeError::Unsatisfiable)?;

        Ok(ResolvedRange { start, end, length })
    }
}

fn parse_decimal(value: &str) -> Result<u64, RangeError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RangeError::Malformed);
    }
    value.parse().map_err(|_| RangeError::Malformed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedRange {
    start: u64,
    end: u64,
    length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RangeError {
    Malformed,
    Unsatisfiable,
}

/// Parsed representation of the optional Range header. Parse errors are retained until the
/// file length is known so a standards-compliant `Content-Range: bytes */length` can be returned.
pub struct ByteRangeRequest(Result<Option<ByteRange>, RangeError>);

impl FromRequestParts<ServerState> for ByteRangeRequest {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(parse_range_headers(&parts.headers)))
    }
}

fn parse_range_headers(headers: &HeaderMap) -> Result<Option<ByteRange>, RangeError> {
    let mut values = headers.get_all(header::RANGE).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(RangeError::Malformed);
    }

    let value = value.to_str().map_err(|_| RangeError::Malformed)?;
    ByteRange::parse(value).map(Some)
}

fn range_not_satisfiable(file_length: u64) -> Response {
    Response::builder()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_RANGE, format!("bytes */{file_length}"))
        .header(header::CONTENT_LENGTH, 0)
        .body(Body::empty())
        .expect("static range error response is valid")
}

fn audio_content_type(path: &FilePath) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("aac") => "audio/aac",
        Some("flac") => "audio/flac",
        Some("m4a" | "mp4") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("oga" | "ogg") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

pub async fn stream_track(
    _session: Session,
    byte_range: ByteRangeRequest,
    State(catalog_store): State<GuardedCatalogStore>,
    State(organic_indexer): State<OptionalOrganicIndexer>,
    Path(id): Path<String>,
) -> Response {
    // Queue track for organic search index expansion
    if let Some(indexer) = &organic_indexer {
        indexer.touch_track(&id);
    }

    // Get track metadata
    let track = match catalog_store.get_track(&id) {
        Ok(Some(track)) => track,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    debug!("Streaming track: {}", track.name);

    // Open through the catalog's root-confined resolver. This prevents catalog
    // paths and symlinks from escaping the configured media directory.
    let (file, path) = match catalog_store.open_track_audio_file(&id) {
        Ok(None) => {
            debug!("Track {} audio not available", track.name);
            return StatusCode::NOT_FOUND.into_response();
        }
        Ok(Some(opened)) => opened,
        Err(error) => {
            debug!(%error, track_id = %id, "Refused or failed to open track audio");
            return StatusCode::NOT_FOUND.into_response();
        }
    };
    debug!("Streaming track from path {}", path.display());

    let mut file = File::from_std(file);
    let file_length = match file.metadata().await {
        Ok(metadata) => metadata.len(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let resolved_range = match byte_range.0 {
        Ok(None) => None,
        Ok(Some(range)) => match range.resolve(file_length) {
            Ok(range) => Some(range),
            Err(_) => return range_not_satisfiable(file_length),
        },
        Err(_) => return range_not_satisfiable(file_length),
    };

    let (status, start, content_length) = match resolved_range {
        Some(range) => (StatusCode::PARTIAL_CONTENT, range.start, range.length),
        None => (StatusCode::OK, 0, file_length),
    };

    if start != 0 && file.seek(SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // `take` is essential: without it a bounded range continues reading until EOF.
    let file_reader = BufReader::with_capacity(STREAM_BUFFER_SIZE, file).take(content_length);
    let stream = ReaderStream::with_capacity(file_reader, STREAM_BUFFER_SIZE);
    let body = Body::from_stream(stream);

    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, audio_content_type(&path))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, content_length);

    if let Some(range) = resolved_range {
        response = response.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", range.start, range.end, file_length),
        );
    }

    response
        .body(body)
        .expect("stream response headers are valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::path::PathBuf;

    fn inclusive(start: u64, end: u64) -> ByteRange {
        ByteRange::Inclusive { start, end }
    }

    fn resolved(start: u64, end: u64) -> ResolvedRange {
        ResolvedRange {
            start,
            end,
            length: end - start + 1,
        }
    }

    #[test]
    fn parses_supported_single_ranges() {
        let cases = [
            ("bytes=0-0", inclusive(0, 0)),
            ("bytes=11-111", inclusive(11, 111)),
            ("bytes=11-", ByteRange::From { start: 11 }),
            ("bytes=-111", ByteRange::Suffix { length: 111 }),
            ("BYTES=3-4", inclusive(3, 4)),
            (
                " bytes=0001-0002 ",
                ByteRange::Inclusive { start: 1, end: 2 },
            ),
            ("bytes=0-18446744073709551615", inclusive(0, u64::MAX)),
        ];

        for (input, expected) in cases {
            assert_eq!(ByteRange::parse(input), Ok(expected), "input: {input}");
        }
    }

    #[test]
    fn rejects_malformed_or_unsupported_ranges() {
        let cases = [
            "",
            "asd",
            "items=0-1",
            "bytes=",
            "bytes=-",
            "bytes=--",
            "bytes=1-2-3",
            "bytes=1 -2",
            "bytes=1- 2",
            "bytes=+1-2",
            "bytes=-1-2",
            "bytes=1-a",
            "bytes=0-1,2-3",
            "bytes=18446744073709551616-",
        ];

        for input in cases {
            assert_eq!(
                ByteRange::parse(input),
                Err(RangeError::Malformed),
                "input: {input}"
            );
        }
        assert_eq!(ByteRange::parse("bytes=-0"), Err(RangeError::Unsatisfiable));
    }

    #[test]
    fn resolves_closed_ranges_and_clamps_end_to_eof() {
        let cases = [
            (inclusive(0, 0), resolved(0, 0)),
            (inclusive(0, 99), resolved(0, 99)),
            (inclusive(50, 99), resolved(50, 99)),
            (inclusive(50, 500), resolved(50, 99)),
            (inclusive(99, u64::MAX), resolved(99, 99)),
        ];

        for (range, expected) in cases {
            assert_eq!(range.resolve(100), Ok(expected), "range: {range:?}");
        }
    }

    #[test]
    fn resolves_open_ended_ranges() {
        assert_eq!(
            ByteRange::From { start: 0 }.resolve(100),
            Ok(resolved(0, 99))
        );
        assert_eq!(
            ByteRange::From { start: 99 }.resolve(100),
            Ok(resolved(99, 99))
        );
    }

    #[test]
    fn resolves_suffix_ranges_from_the_end() {
        let cases = [
            (1, resolved(99, 99)),
            (50, resolved(50, 99)),
            (100, resolved(0, 99)),
            (101, resolved(0, 99)),
            (u64::MAX, resolved(0, 99)),
        ];

        for (length, expected) in cases {
            assert_eq!(
                ByteRange::Suffix { length }.resolve(100),
                Ok(expected),
                "suffix length: {length}"
            );
        }
    }

    #[test]
    fn rejects_unsatisfiable_resolved_ranges() {
        let cases = [
            inclusive(50, 49),
            inclusive(100, 100),
            inclusive(u64::MAX, u64::MAX),
            ByteRange::From { start: 100 },
            ByteRange::From { start: u64::MAX },
            ByteRange::Suffix { length: 0 },
        ];

        for range in cases {
            assert_eq!(
                range.resolve(100),
                Err(RangeError::Unsatisfiable),
                "range: {range:?}"
            );
        }
    }

    #[test]
    fn every_range_is_unsatisfiable_for_an_empty_file() {
        let cases = [
            inclusive(0, 0),
            ByteRange::From { start: 0 },
            ByteRange::Suffix { length: 1 },
        ];

        for range in cases {
            assert_eq!(range.resolve(0), Err(RangeError::Unsatisfiable));
        }
    }

    #[test]
    fn maximum_u64_file_length_does_not_overflow() {
        assert_eq!(
            inclusive(0, u64::MAX).resolve(u64::MAX),
            Ok(ResolvedRange {
                start: 0,
                end: u64::MAX - 1,
                length: u64::MAX,
            })
        );
        assert_eq!(
            ByteRange::Suffix { length: u64::MAX }.resolve(u64::MAX),
            Ok(ResolvedRange {
                start: 0,
                end: u64::MAX - 1,
                length: u64::MAX,
            })
        );
        assert_eq!(
            ByteRange::From {
                start: u64::MAX - 1,
            }
            .resolve(u64::MAX),
            Ok(ResolvedRange {
                start: u64::MAX - 1,
                end: u64::MAX - 1,
                length: 1,
            })
        );
    }

    #[test]
    fn exhaustive_small_ranges_have_consistent_bounds_and_lengths() {
        for file_length in 0_u64..=32 {
            for start in 0_u64..=40 {
                for requested_end in 0_u64..=40 {
                    let result = inclusive(start, requested_end).resolve(file_length);
                    let should_succeed =
                        file_length > 0 && start <= requested_end && start < file_length;

                    assert_eq!(result.is_ok(), should_succeed);
                    if let Ok(range) = result {
                        assert_eq!(range.start, start);
                        assert_eq!(range.end, requested_end.min(file_length - 1));
                        assert!(range.start <= range.end);
                        assert!(range.end < file_length);
                        assert_eq!(range.length, range.end - range.start + 1);
                    }
                }

                let from = ByteRange::From { start }.resolve(file_length);
                assert_eq!(from.is_ok(), file_length > 0 && start < file_length);
                if let Ok(range) = from {
                    assert_eq!(range.start, start);
                    assert_eq!(range.end, file_length - 1);
                    assert_eq!(range.length, file_length - start);
                }

                let suffix = ByteRange::Suffix { length: start }.resolve(file_length);
                assert_eq!(suffix.is_ok(), file_length > 0 && start > 0);
                if let Ok(range) = suffix {
                    assert_eq!(range.start, file_length.saturating_sub(start));
                    assert_eq!(range.end, file_length - 1);
                    assert_eq!(range.length, start.min(file_length));
                }
            }
        }
    }

    #[test]
    fn parses_absent_single_and_duplicate_header_fields() {
        let mut headers = HeaderMap::new();
        assert_eq!(parse_range_headers(&headers), Ok(None));

        headers.insert(header::RANGE, HeaderValue::from_static("bytes=2-3"));
        assert_eq!(parse_range_headers(&headers), Ok(Some(inclusive(2, 3))));

        headers.append(header::RANGE, HeaderValue::from_static("bytes=5-6"));
        assert_eq!(parse_range_headers(&headers), Err(RangeError::Malformed));
    }

    #[test]
    fn range_error_response_has_required_headers_and_no_body_length() {
        let response = range_not_satisfiable(1234);

        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
        assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */1234");
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
    }

    #[test]
    fn maps_supported_audio_extensions_without_case_sensitivity() {
        let cases = [
            ("track.ogg", "audio/ogg"),
            ("track.OPUS", "audio/opus"),
            ("track.mp3", "audio/mpeg"),
            ("track.flac", "audio/flac"),
            ("track.wav", "audio/wav"),
            ("track.m4a", "audio/mp4"),
            ("track.aac", "audio/aac"),
            ("track.unknown", "application/octet-stream"),
            ("track", "application/octet-stream"),
        ];

        for (path, expected) in cases {
            assert_eq!(audio_content_type(&PathBuf::from(path)), expected);
        }
    }
}
