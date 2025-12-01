## [catalog-server]

### [ready for coding]

- Wire up the external downloader.

### [to refine]

- Consider switching Docker deployment to use a separate nginx service for web frontend serving (Option C) instead of serving from catalog-server. This would allow independent frontend deployments and better static file optimization.

- Plan how are we going to put our catalog agent. This agent has the responsability to provide information to users, and if the info is not available yet in the catalog, it should be able to find it in external sources

### [done]

- ~Once catalog is migrated to db and catalog modification endpoints are in place, create a catalog change log to be shown to users, see CHANGELOG_FEATURE_PLAN.md for details.~
- ~We should plan a feature to gather listening stats from users, see LISTENING_STATS_PLAN.md for details.~
- ~Add display_image_id to Artist and Album models (schema v1 migration) with Python script for populating from largest image~
- ~Verify that web interface works with the docker setup~
- ~Complete SQLite catalog migration (Phase 1-6) - See MIGRATION_PROGRESS.md~
- ~Create catalog db (SQLite-backed CatalogStore with --catalog-db CLI flag)~
- ~Implement catalog editing endpoints (EditCatalog permission) - CRUD for artists, albums, tracks, images~
- ~Add validation for catalog write operations (transactional, foreign key checks)~
- ~Remove legacy filesystem catalog code (now SQLite-only)~
- ~Implement bandwidth usage statistics collection and persistence (track data transfer per user/endpoint)~
- ~Create admin API endpoints for managing user roles and permissions (ManagePermissions permission)~
- ~Create custom, flashy, modern looking CLI interface style for cli-auth~
- ~Secure /metrics endpoint (add authentication, separate port, or network-level restrictions)~
- ~Setup alerts infrastructure (emails or telegram bot for rate limit violations, errors, etc.)~
- ~Setup metrics infrastructure for monitoring and observability~
- ~Add unit tests for permissions system (to_int/from_int, UserRole permissions, permission grants)~
- ~Add unit tests for session management (token extraction, permission checking, invalid tokens)~
- ~Add unit tests for catalog loading error cases (missing files, invalid references, problem accumulation)~
- ~Add unit tests for rate limiting behavior~
- ~Check UserStore return type, we should not swallow errors~
- ~Implement server reboot endpoint (RebootServer permission)~
- ~Set last used to auth token~
- ~Verify that all sqlite operations are performed within a transaction~
- ~Implement a rate limiting of some sort~
- ~Add extra permission management to cli-auth (time-based and countdown-based grants)~
- ~Delete cli_search and clean up unused stuff and format and warnings~
- ~Add user roles~
- ~Update cli-auth to include user roles~
- ~Add hard limit for playlist size (150?)~
- ~Add user playlists~
- ~Make no_checks a cli args rather than build feature~
- ~Add cache directive to responses~
- ~Wrap ids so that the type can be embedded in them~
- ~Add user saved albums, tracks and artists~
- ~Show requests in logs~
- ~Create User identity/authentication db~
