# One-off Work graph extraction

Required follow-up stage for `.local-work-enrichment/album-data-extraction/`:

```sh
python3 scripts/work-enrichment/complete_work_graph.py
python3 -m unittest discover -s scripts/work-enrichment -p 'test_*.py'
```

Needs the existing MusicBrainz `20260912-002318` dump, schema-31 raw tables, and the recording-linked `works.jsonl`/`evidence.sqlite` extraction. Use `--workspace PATH` to point to another workspace with this layout. It extracts missing relationship tables, follows all Work-to-Work connections recursively, loads every endpoint and its artist relationships, validates foreign keys and updates the local evidence database and manifest. It never contacts production.

Preserves relationship type, directed endpoints, ordering, dates, attributes and credits. `parts` can represent a collection and movements; arrangement/version/quotation links retain their separate semantics. Related Works without catalog performances remain graph nodes, not new track assignments. Cycles terminate via visited-node tracking. No depth cap silently truncates the graph.

Artifacts are in `album-data-extraction/work-graph/`. `COMPLETE` plus the checked seed hash makes completed reruns idempotent. Interrupted graph builds fail visibly rather than overwrite an existing build; retain their directory for diagnosis and rebuild into a fresh extraction. The missing-table extractor writes via `.partial` files and atomic renames.

The import bundle includes both the expanded Works and directed relationships. Production Work storage/import still needs graph support before this bundle is applied. Track links must keep their performed child Work; parent or arrangement nodes must not replace it.

## Production importer

`import_work_bundle.py` imports a checksummed `production-manifest.json` bundle. The bundle contains graph `works.jsonl`, `relationships.jsonl`, the reviewed `staged-link-batch.json`, `track-evidence.jsonl`, and the shared server schema `work_graph.sql`. The manifest lists every input file's SHA-256 and the source snapshot. Dry-run is the default and applies the transaction to a consistent SQLite backup copy:

```sh
python3 scripts/work-enrichment/import_work_bundle.py \
  --bundle /path/to/bundle --db /path/to/enrichment.db \
  --catalog /path/to/catalog.db --report dry-run.json
```

After a successful dry run, stop writers to the enrichment database and use `--apply --backup /new/backup/path.db`. A new backup path is mandatory; existing backups are never overwritten. Restart the service even if the import fails. The import creates graph storage additively; a new server binary is not required to read the existing flat Work/track tables. Future server startup uses the same SQL schema. API/UI graph traversal is a separate concern from persistence.

The importer checks current catalog availability, album identity and exact duration against the fingerprint snapshot, preserves existing track links and externally identified Work records, and reports conflicting links. Source IDs distinguish same-named Works. Graph relationships retain typed direction/order and all original evidence. Creator claims never update catalog artist credits. Every import is one transaction; bundle IDs prevent duplicate application. Errors roll back the transaction. Rejected/review-only track links are not part of this import.
