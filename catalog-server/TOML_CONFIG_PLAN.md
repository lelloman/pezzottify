# TOML Configuration System Plan

## Overview

Replace CLI arguments with a TOML configuration file for the catalog-server. This provides a more maintainable and flexible way to configure the server, especially as we add more features with configurable parameters.

## TODO: Spec this out

### Topics to Cover

- [ ] Configuration file location and discovery
- [ ] Schema definition (all configurable values)
- [ ] Migration from CLI arguments
- [ ] Default values
- [ ] Environment variable overrides
- [ ] Validation and error handling
- [ ] Hot-reload support (if needed)
- [ ] Example configuration file

### Current CLI Arguments to Migrate

```
<catalog-db-path>
<user-db-path>
--media-path <PATH>
--port <PORT>
--metrics-port <PORT>
--logging-level <LEVEL>
--content-cache-age-sec <SECONDS>
--frontend-dir-path <PATH>
--downloader-url <URL>
--downloader-timeout-sec <SECONDS>
```

### New Configuration Values (Download Manager)

```
[download_manager]
enabled = true
max_albums_per_hour = 10
max_albums_per_day = 60
user_max_requests_per_day = 100
user_max_queue_size = 200
```

### Dependencies

- None (foundational feature)

### Used By

- Download Manager
- Background Jobs System
- Future features
