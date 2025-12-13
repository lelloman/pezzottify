#!/usr/bin/env python3
"""
Mock downloader service for integration tests.

This provides a minimal implementation of the downloader API that the catalog-server
expects when the download manager feature is enabled.

Endpoints:
- GET /health - Health check
- GET /status - Service status
- GET /search?q=<query>&type=<album|artist> - Search for content
- GET /artist/{id} - Artist metadata
- GET /artist/{id}/discography - Artist's album IDs
- GET /album/{id} - Album metadata
- GET /album/{id}/tracks - Album tracks
- GET /track/{id} - Track metadata
- GET /track/{id}/audio - Track audio (returns dummy data)
- GET /image/{id} - Image (returns dummy data)
"""

import json
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

# Test data - matches the catalog created by run-integration-tests.sh
TEST_ARTIST_ID = "ext-artist-prince"
TEST_ALBUM_ID = "ext-album-purple-rain"
TEST_TRACK_ID = "ext-track-purple-rain"
TEST_IMAGE_ID = "ext-image-001"

# Mock data
MOCK_ARTISTS = {
    TEST_ARTIST_ID: {
        "id": TEST_ARTIST_ID,
        "name": "Prince (External)",
        "genre": ["Funk", "Pop", "Rock"],
        "portraits": [
            {"id": TEST_IMAGE_ID, "size": "large", "width": 640, "height": 640}
        ],
        "activity_periods": [],
        "related": [],
        "portrait_group": []
    }
}

MOCK_ALBUMS = {
    TEST_ALBUM_ID: {
        "id": TEST_ALBUM_ID,
        "name": "Purple Rain (External)",
        "album_type": "album",
        "artists_ids": [TEST_ARTIST_ID],
        "label": "Warner Bros.",
        "date": 456789012,
        "genres": ["Funk", "Pop"],
        "covers": [
            {"id": TEST_IMAGE_ID, "size": "large", "width": 640, "height": 640}
        ],
        "discs": [
            {"number": 1, "name": "", "tracks": [TEST_TRACK_ID]}
        ],
        "related": [],
        "cover_group": [],
        "original_title": None,
        "version_title": "",
        "type_str": "album"
    }
}

MOCK_TRACKS = {
    TEST_TRACK_ID: {
        "id": TEST_TRACK_ID,
        "name": "Purple Rain (External)",
        "album_id": TEST_ALBUM_ID,
        "artists_ids": [TEST_ARTIST_ID],
        "number": 1,
        "disc_number": 1,
        "duration": 520000,
        "is_explicit": False,
        "files": {"flac": "abc123"},
        "alternatives": [],
        "tags": [],
        "earliest_live_timestamp": None,
        "has_lyrics": False,
        "language_of_performance": ["en"],
        "original_title": None,
        "version_title": "",
        "artists_with_role": [
            {"artist_id": TEST_ARTIST_ID, "name": "Prince (External)", "role": "main"}
        ]
    }
}

MOCK_DISCOGRAPHIES = {
    TEST_ARTIST_ID: {
        "albums": [TEST_ALBUM_ID]
    }
}


class MockDownloaderHandler(BaseHTTPRequestHandler):
    """HTTP request handler for the mock downloader service."""

    def log_message(self, format, *args):
        """Log requests to stderr for debugging."""
        sys.stderr.write(f"[mock-downloader] {args[0]}\n")

    def send_json(self, data, status=200):
        """Send a JSON response."""
        body = json.dumps(data).encode('utf-8')
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', len(body))
        self.end_headers()
        self.wfile.write(body)

    def send_error_json(self, status, message):
        """Send a JSON error response."""
        self.send_json({"error": message}, status)

    def send_binary(self, data, content_type):
        """Send a binary response."""
        self.send_response(200)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', len(data))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        """Handle GET requests."""
        parsed = urlparse(self.path)
        path = parsed.path
        query = parse_qs(parsed.query)

        # Health check
        if path == '/health':
            self.send_json({"status": "ok"})
            return

        # Status
        if path == '/status':
            self.send_json({
                "status": "ready",
                "version": "mock-1.0.0"
            })
            return

        # Search
        if path == '/search':
            search_query = query.get('q', [''])[0].lower()
            search_type = query.get('type', ['album'])[0]

            results = {"albums": {"items": []}, "artists": {"items": []}}

            # Return mock results if query matches our test data
            if 'prince' in search_query or 'purple' in search_query:
                if search_type in ['album', 'all']:
                    results["albums"]["items"].append({
                        "id": TEST_ALBUM_ID,
                        "name": "Purple Rain (External)",
                        "album_type": "album",
                        "total_tracks": 1,
                        "release_date": "1984-06-25",
                        "images": [{"url": f"http://localhost:8090/image/{TEST_IMAGE_ID}", "width": 640, "height": 640}],
                        "artists": [{"id": TEST_ARTIST_ID, "name": "Prince (External)"}]
                    })
                if search_type in ['artist', 'all']:
                    results["artists"]["items"].append({
                        "id": TEST_ARTIST_ID,
                        "name": "Prince (External)",
                        "genres": ["Funk", "Pop"],
                        "images": [{"url": f"http://localhost:8090/image/{TEST_IMAGE_ID}", "width": 640, "height": 640}]
                    })

            self.send_json(results)
            return

        # Artist endpoints
        if path.startswith('/artist/'):
            parts = path.split('/')
            artist_id = parts[2] if len(parts) > 2 else None

            if not artist_id:
                self.send_error_json(400, "Missing artist ID")
                return

            # Discography
            if len(parts) > 3 and parts[3] == 'discography':
                if artist_id in MOCK_DISCOGRAPHIES:
                    self.send_json(MOCK_DISCOGRAPHIES[artist_id])
                else:
                    self.send_error_json(404, "Artist not found")
                return

            # Artist metadata
            if artist_id in MOCK_ARTISTS:
                self.send_json(MOCK_ARTISTS[artist_id])
            else:
                self.send_error_json(404, "Artist not found")
            return

        # Album endpoints
        if path.startswith('/album/'):
            parts = path.split('/')
            album_id = parts[2] if len(parts) > 2 else None

            if not album_id:
                self.send_error_json(400, "Missing album ID")
                return

            # Album tracks
            if len(parts) > 3 and parts[3] == 'tracks':
                if album_id in MOCK_ALBUMS:
                    track_ids = MOCK_ALBUMS[album_id]["discs"][0]["tracks"]
                    tracks = [MOCK_TRACKS[tid] for tid in track_ids if tid in MOCK_TRACKS]
                    self.send_json(tracks)
                else:
                    self.send_error_json(404, "Album not found")
                return

            # Album metadata
            if album_id in MOCK_ALBUMS:
                self.send_json(MOCK_ALBUMS[album_id])
            else:
                self.send_error_json(404, "Album not found")
            return

        # Track endpoints
        if path.startswith('/track/'):
            parts = path.split('/')
            track_id = parts[2] if len(parts) > 2 else None

            if not track_id:
                self.send_error_json(400, "Missing track ID")
                return

            # Track audio
            if len(parts) > 3 and parts[3] == 'audio':
                # Return dummy FLAC-like data
                dummy_audio = b'fLaC' + b'\x00' * 1000
                self.send_binary(dummy_audio, 'audio/flac')
                return

            # Track metadata
            if track_id in MOCK_TRACKS:
                self.send_json(MOCK_TRACKS[track_id])
            else:
                self.send_error_json(404, "Track not found")
            return

        # Image endpoint
        if path.startswith('/image/'):
            # Return a minimal valid JPEG
            # This is a 1x1 red pixel JPEG
            dummy_jpeg = bytes([
                0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
                0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
                0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
                0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
                0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
                0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
                0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
                0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
                0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00,
                0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0x09, 0x0A, 0x0B, 0xFF, 0xC4, 0x00, 0xB5, 0x10, 0x00, 0x02, 0x01, 0x03,
                0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
                0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06,
                0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
                0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72,
                0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
                0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45,
                0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
                0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75,
                0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
                0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3,
                0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
                0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9,
                0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
                0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
                0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01,
                0x00, 0x00, 0x3F, 0x00, 0xFB, 0xD5, 0xDB, 0x20, 0xA8, 0xF1, 0x5E, 0x5A,
                0xE9, 0x7F, 0xFF, 0xD9
            ])
            self.send_binary(dummy_jpeg, 'image/jpeg')
            return

        # Unknown endpoint
        self.send_error_json(404, f"Unknown endpoint: {path}")


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8090
    server = HTTPServer(('0.0.0.0', port), MockDownloaderHandler)
    print(f"Mock downloader service running on port {port}", file=sys.stderr)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down mock downloader service", file=sys.stderr)
        server.shutdown()


if __name__ == '__main__':
    main()
