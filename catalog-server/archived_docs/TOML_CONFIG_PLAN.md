# TOML Configuration System Plan

## Overview

Replace CLI arguments with a TOML configuration file for the catalog-server. This provides a more maintainable and flexible way to configure the server, especially as we add more features with configurable parameters.

---

## Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| TOML file support | ✅ Complete | `--config <PATH>` argument implemented |
| CLI argument migration | ✅ Complete | Positional args removed, `--db-dir` implemented |
| Config module | ✅ Complete | `catalog-server/src/config/` module |
| TOML overrides CLI | ✅ Complete | TOML values take precedence over CLI defaults |

---

## Current Implementation

The TOML configuration system is fully implemented. Configuration can be provided via:

1. **TOML config file** (recommended): `--config ./config.toml`
2. **CLI arguments**: Individual `--<option>` flags
3. **Combination**: CLI provides defaults, TOML overrides

### Supported Configuration Options

```toml
# Server configuration
db_dir = "/path/to/db"           # Directory containing .db files
media_path = "/path/to/media"    # Path to media files (audio/images)
port = 3001                      # HTTP server port
metrics_port = 9091              # Prometheus metrics port
logging_level = "path"           # Request logging level
content_cache_age_sec = 3600     # HTTP cache duration
frontend_dir_path = "/path/to/frontend"  # Static frontend files

# Downloader integration (optional)
downloader_url = "http://downloader:8080"
downloader_timeout_sec = 300

# Event pruning
event_retention_days = 30        # Days to retain sync events (0 = disable)
prune_interval_hours = 24        # Interval between pruning runs
```

### Example Usage

```bash
# Using config file (recommended)
cargo run -- --config ./config.toml

# Using CLI arguments
cargo run -- --db-dir ./data --media-path ./media --port 3001

# Mixed (CLI provides base, config overrides)
cargo run -- --db-dir ./data --config ./overrides.toml
```

---

## Future Enhancements (Not Yet Implemented)

### Topics for Future Consideration

- [ ] Environment variable overrides (e.g., `PEZZOTTIFY_PORT`)
- [ ] Hot-reload support for configuration changes
- [ ] Configuration validation with detailed error messages
- [ ] Download Manager configuration section (when implemented)

### Planned Download Manager Configuration

```toml
[download_manager]
enabled = true
max_albums_per_hour = 10
max_albums_per_day = 60
user_max_requests_per_day = 100
user_max_queue_size = 200
```

---

## Dependencies

- None (foundational feature)

## Used By

- Background Jobs System
- Future features (Download Manager, etc.)
