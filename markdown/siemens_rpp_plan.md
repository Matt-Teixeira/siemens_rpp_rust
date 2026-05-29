# Siemens RPP — Rust Port Technical Plan

## Status

- **Phase:** Planning / pre-scaffold. No code written.
- **Last updated:** 2026-05-29
- **Source app being replaced:** `~/apps/hhm_rpp_siemens` (Node)
- **Target app:** the app named `siemens_rpp`, living in the `~/apps/siemens_rpp_rust` directory (Rust). The directory carries the `_rust` suffix to disambiguate from the Node app; the binary/crate/image/`APP_NAME` stay `siemens_rpp` (part of the log + run-path contract). See [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md) TD-019. Scaffolding/workflow docs live in `markdown/`; per-phase AI handoffs in `notes/`.
- **Scope:** CT and MRI parsers only. `siemens_cv` is deprecated per project memory. `win_7` parser path deferred to a future port.
- **Why Rust:** the hot path is per-line regex over potentially large log files plus tz-aware datetime construction — a textbook fit. Adjacent benefit: a static binary deploys to a much smaller container than `node:lts` and removes the `npm ci` step from the per-run cost.

## Background

`hhm_rpp_siemens` runs as a cron-triggered job that, for each Siemens system in a PG-driven worklist, opens the system's most recent log file (already `rsync`'d to disk by the `data_acquisition` app), scans top-to-bottom against a regex with named groups, generates a timezone-aware `host_datetime`, batch-inserts rows into `log.siemens_ct` or `log.siemens_mri`, upserts an "offline tracker" row, and writes a Redis "cursor" (the file's first line) so the next run can stop scanning when it reaches the previously-seen head.

The intent is to keep the data contract bit-identical (same PG tables, same Redis keys, same `util.app_run_logs` shape) while replacing the parser app itself.

## Verified current behavior of hhm_rpp_siemens

The data flow below is identical for CT and MRI; only the regex variant and target table differ.

```
boot:
  PG query SIEMENS_CT (or SIEMENS_MRI) from acquisition/on_boot_queries.js
  → list of systems, each carrying:
      { id, manufacturer, modality, time_zone_id, debian_server_path,
        log_config: { file_name, dir_name, parsers, pg_tables, file_version } }

per system:
  build path: ${DATA_STORE_DEV}/${id}/${log_config.file_name}
  Redis GET ${sme}.${file_name}              ← cursor = "most recent first line"
  if !file_exists → log warn, continue
  open readline stream (crlfDelay: Infinity)
  for each line, top→bottom:
      if line_num == 1: first_line = line
      if line == redis_line: break           ← cursor hit, stop
      match line against win_10_re[parsers[0]] (re_v1 or re_v2)
      if null: blankLineTest → continue; else log warn + (latent Node bug, see Risks)
      generate host_datetime via luxon (see "Time correctness")
      push row { system_id, host_state, host_date, host_time, source_group,
                 type_group, text_group, capture_datetime, host_datetime }
  if rows: pgp.helpers.insert into log.siemens_ct (or log.siemens_mri)
  upsert alert.offline_hhm_conn(system_id, rpp_host_datetime = rows[0].host_datetime)
  Redis SET ${sme}.${file_name} = first_line
  CT-only: extract scan-seconds metadata sidecar (deferred to v1.1)
  gzip the whole file, INSERT INTO log.saved_files(system_id, file_name, buffer, capture_datetime)

end of run:
  serialize log_events array → INSERT into util.app_run_logs
  write same array to /opt/run-logs/${APP_NAME}/${APP_NAME}-log.${LOGGER}.${run_id}.json
```

### PG target columns

From `hhm_rpp_siemens/utils/db/sql/pg-helpers_hhm.js`:

```
log.siemens_ct / log.siemens_mri:
  system_id, host_state, host_date, host_time, source_group, type_group,
  text_group, domain_group, id_group, month, day, year,
  host_datetime, capture_datetime
```

**Note:** `domain_group`, `id_group`, `month`, `day`, `year` are in the schema but **not** captured by the win_10 regex. They are NULL in production rows today via `mapDataToSchema` filling missing keys with `null`. The Rust port replicates this exactly.

### Parser regexes (from `hhm_rpp_siemens/parse/parsers.js`)

Both are TSV-shaped, only `host_time` and `host_state` swap:

```
re_v1: <host_state>\t<host_date>\t<host_time>\t<source_group>\t<type_group>\t<text_group>
re_v2: <host_time>\t<host_state>\t<host_date>\t<source_group>\t<type_group>\t<text_group>
```

`file_config.parsers[0]` from PG picks which one a system uses.

## Architecture decisions made

### Contract 1: Redis cache key

Key shape: `${sme}.${file_name}` — e.g. `SME00817.EvtApplication_Today.txt`. Value is the file's first line as a raw string, used as a cursor.

- Connection: `socket: { host: REDIS_HOST, port: REDIS_PORT }`, no auth, no TLS, no db selector.
- The Rust app must read and write this key by the same convention.
- Cursor value must be stored **byte-for-byte** — no trimming, no normalization, no decode-then-re-encode. CRLF handling is the dominant parity risk (see Open Decision #5 and Risks).

### Contract 2: File path root

The new fixed root is `/opt/resources/acqu_files/`. The current Node code uses `${DATA_STORE_DEV}/${id}/${file_name}` — a single concatenation.

A `PathBuilder` strategy will be keyed on `(manufacturer, modality)` and populated from `log_config`.

**Resolved (Open Decision #1 → TD-015):** for siemens the shape is the **first**
candidate — `/opt/resources/acqu_files/<system_id>/<file_name>` (`dir_name` is not
used for siemens). In the Rust app the root is configured as **`ACQU_FILES_ROOT`**
(renamed from Node's `DATA_STORE_DEV`; identical value). The candidates considered:

```
/opt/resources/acqu_files/<sme>/<file_name>              ← chosen (TD-015)
/opt/resources/acqu_files/<dir_name>/<sme>/<file_name>
/opt/resources/acqu_files/<sme>/<dir_name>/<file_name>
```

CLI accepts `--file <path>` for testing and one-off retries.

### Contract 3: Time correctness

- **Library:** `chrono` + `chrono-tz`.
- **Format:** luxon `yyyy-MM-ddHH:mm:ss` maps to chrono `"%Y-%m-%d%H:%M:%S"` (strict, zero-padded). See Open Decision #2 about confirming production padding.
- **Timezone source:** `sites.time_zone_id` from the boot-query row, IANA name → `chrono_tz::Tz::from_str`. Fall back to `America/New_York` if NULL (matches `generateDateTime` default in `processing/generateDateTimes.js`).
- **Output:** chrono `.to_rfc3339()`. PG `timestamptz` accepts this byte-shape identically to luxon's `.toISO()`. Worth a grep of downstream consumers for any string-comparison code before final cutover (luxon emits `…-05:00` with `.000` milliseconds; chrono emits `…-05:00` with no `.000`).
- **`capture_datetime`:** computed once per run as `Utc::now().with_timezone(&America::New_York).to_rfc3339()` — matches `tooling/dates.js#dt_now`.
- **DST ambiguous-hour policy:** luxon defaults to "earliest" occurrence; chrono returns `LocalResult::Ambiguous` and forces an explicit choice. v1 will match luxon (`.earliest()`) to avoid silent data drift. See Open Decision #3.
- **Parity test (required before cutover):** take ~1000 prod rows from `log.siemens_ct`, run both luxon and the Rust function over the source `(host_date, host_time, tz)`, assert byte-equal output.

### Contract 4: Logging

The Node logger contract (from `utils/logger/log.js`) is reproducible bit-identically in Rust — **no Rust-specific log table needed.**

```
in-memory: array of
  { run_id, dt: ISO, type: INFO|WARN|ERROR, func,
    tag: CALL|DETAILS|CATCH|SEQUENCE HALTED|QA FAILURE,
    note: <arbitrary JSON object>,
    err_msg?: <stack or stringified error> }

end-of-run, one INSERT into util.app_run_logs:
  ( app_name, run_id,
    verbose_log    = JSON.stringify(events),
    warn_error_logs = JSON.stringify(events filtered to WARN+ERROR) )

end-of-run, one file write:
  /opt/run-logs/${APP_NAME}/${APP_NAME}-log.${LOGGER}.${run_id}.json
```

A shared crate `rpp_log` will encapsulate this and be reused by every future Rust app in the fleet. Event types and tags stay as the existing uppercase strings (`INFO`/`WARN`/`ERROR`, `CALL`/`DETAILS`/`CATCH`/...) so existing dashboards keep working. `note` is `serde_json::Value` so the JSON shape matches Node's `JSON.stringify(note)` semantics. A `tracing` Layer is exposed on top of this for dev ergonomics; the buffered events sent to PG remain the source of truth.

See Open Decision #4 about the byte-exact-vs-typed serialization of `note`.

## Architecture decisions still open

> **Update 2026-05-29 (source-verified against `~/apps/hhm_rpp_siemens`):** the
> four scaffold-blocking decisions are now **resolved** — #1, #5, #6, #8 — plus #2
> is informed by the regex. Full rationale with file:line citations is in
> [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md) TD-015…TD-019. Summary:
> - **#1 file path** → `/opt/resources/acqu_files/<system_id>/<file_name>` (the
>   first candidate; `dir_name` unused for siemens). The root is exposed as
>   **`ACQU_FILES_ROOT`** in the Rust `.env` (renamed from Node's `DATA_STORE_DEV`;
>   same value, name only). [TD-015]
> - **#5 CRLF** → strip trailing `\r` symmetrically on read+write to emulate Node
>   `readline`. [TD-016]
> - **#6 bad-match** → **warn-and-skip** (documented divergence: Node actually
>   throws → system-level catch → no insert + cursor not advanced, wedging that
>   system). [TD-017]
> - **#8 invocation** → native subcommand, Option A. [TD-018]
> - **#2 date format** → the win_10 regex allows single-digit month/day
>   (`\d{1,2}`), so dates are **not** guaranteed zero-padded; date+time are
>   concatenated with **no** separator. Handle deliberately in Phase 2. [TD-005]
>
> The remaining open items below (#3, #4, #7, #9, #10) are non-blocking and decided
> at their phase.

Numbered for handoff — each blocks scaffolding in some way.

1. **File path construction.** Which of the three candidate shapes under `/opt/resources/acqu_files/` is correct for siemens specifically? Resolve by inspecting one production PG row's `log_config.dir_name` and confirming with the user how siemens places files under the new root.
2. **Date format strictness.** Production `host_date` confirmed always zero-padded `YYYY-MM-DD`? If sometimes single-digit, format becomes `%Y-%-m-%-d`.
3. **DST ambiguous-hour policy.** Match luxon's earliest default, or pick a different explicit policy? Default recommendation: earliest.
4. **`note` field serialization.** Byte-match Node's `JSON.stringify` (key order, types), or allowed to drift to typed serde shape? Affects whether downstream log parsers break.
5. **CRLF cursor handling.** Are production Siemens log files CRLF or LF? Drives whether `trim_end_matches('\r')` is required on the cursor compare (symmetric on read + write). This is the top behavioral parity risk.
6. **Bad-match Node bug.** `jobs/win_10/siemens_ct.js:84-107` has a latent crash when regex returns `null` on a non-blank line: warn logs, then `matches.groups.system_id = ...` would throw on `null.groups`. Rust port: replicate exactly, or fix to "warn-and-skip"? Recommend the latter and instrument the count.
7. **Shadow tables.** Cutover plan uses `log.siemens_ct_shadow` / `log.siemens_mri_shadow` for phase 3. Should we write the migrations now, use a different schema, or skip shadow and rely on the offline parity harness?
8. **Invocation syntax.** Current Node apps run as `docker compose run --rm app_tools bash -lc "npm run <job_name>"`. Three options:
   - A: `docker compose run --rm app_tools siemens_rpp ct` (natural; recommended; ~3 cron edits)
   - B: keep `bash -lc "npm run siemens_ct"` via a fake `package.json` + `npm` shim
   - C: introduce `make` or `just` targets across the fleet
9. **Runtime base image.** `debian:bookworm-slim` (recommended; allows bash, ~80MB) vs `gcr.io/distroless/cc-debian12` (smaller, no shell, no debug tooling).
10. **User UIDs in image.** Mirror the Node Dockerfile's `svc` / `jonathan-pope` / `matt-teixeira` UIDs verbatim, or simplify to only `svc`? Affects file ownership on the host-mounted `/opt/run-logs/siemens_rpp` dir.

## Crate stack

| Concern | Crate | Notes |
|---|---|---|
| async runtime | `tokio` (`rt-multi-thread`) | only because PG/Redis crates are async; the parse inner loop stays sync |
| PG client | `tokio-postgres` + `deadpool-postgres` pool | `sqlx` compile-time check would require a live DB in CI; `tokio-postgres` more pragmatic here |
| PG bulk insert | `tokio-postgres` multi-row INSERT via `unnest` for v1; consider binary `COPY` later | UNNEST is easier to evolve to ON CONFLICT |
| Redis | `redis` (with `tokio-comp`) | matches existing simple connection contract |
| Regex | `regex` | `(?P<name>...)` syntax — port from Node's `(?<name>...)` |
| Datetime | `chrono` + `chrono-tz` | DST policy per Open Decision #3 |
| JSON | `serde` + `serde_json` | `note` field, PG json columns |
| CLI | `clap` (derive) | replaces `process.argv` dispatch |
| env | `dotenvy` | reads the same `.env` |
| gzip | `flate2` (with `zlib-ng` backend) | replaces zlib `gzipAsync` |
| UUID | `uuid` v4 | `run_id`, `job_id` |
| Errors | `thiserror` (lib) + `anyhow` (bin) | typed per-stage errors, top-level `anyhow` |
| Tracing | `tracing` + `tracing-subscriber` | dev ergonomics; persisted logs go through `rpp_log` |

## Project layout

Cargo workspace shape:

```
~/apps/siemens_rpp/
├── Cargo.toml                        ← workspace root
├── docker-compose.yaml
├── docker/
│   ├── Dockerfile                    ← multi-stage; binary baked in
│   └── entrypoint.sh                 ← copy of data_acquisition's verbatim
├── .env
├── crates/
│   ├── rpp_log/                      ← shared, fleet-wide
│   ├── rpp_db/                       ← shared: PG pool, app_run_logs writer
│   ├── rpp_redis/                    ← shared: connection + typed key helpers
│   └── siemens_rpp/                  ← the binary
│       ├── src/
│       │   ├── main.rs               ← clap CLI, dispatch
│       │   ├── boot.rs               ← run boot query, materialize Vec<SystemRow>
│       │   ├── path.rs               ← PathBuilder strategy
│       │   ├── parse/
│       │   │   ├── win10.rs          ← re_v1 + re_v2 regexes + line→Row
│       │   │   └── blank_line.rs
│       │   ├── time.rs               ← chrono-tz helpers, capture_dt
│       │   ├── persist/
│       │   │   ├── insert.rs         ← multi-row INSERT into log.siemens_*
│       │   │   ├── offline_upsert.rs ← alert.offline_hhm_conn
│       │   │   └── gzip_save.rs      ← log.saved_files
│       │   ├── redis_cursor.rs       ← get/set ${sme}.${file_name}
│       │   └── runner.rs             ← one System: read → parse → persist → cursor
│       └── tests/
│           ├── golden/               ← captured fixtures from prod
│           └── parity.rs             ← cross-check Node JSON outputs
```

The three shared crates (`rpp_log`, `rpp_db`, `rpp_redis`) are designed to be lifted into their own workspace once a second Rust app exists.

## CLI shape

Single binary, subcommands aligned with the Node `npm run` scripts:

```
siemens_rpp ct                     ← runs SIEMENS_CT boot query then per-system loop
siemens_rpp mri                    ← runs SIEMENS_MRI boot query then per-system loop
siemens_rpp parse                  ← single-system mode for testing/one-off retry:
    --system-id SMExxxxx
    --modality ct|mri
    --file /opt/resources/acqu_files/.../EvtApplication_Today.txt
    --tz America/New_York
    [--dry-run]                     ← parse + report, no PG/Redis writes
    [--no-cursor-update]             ← skip redis SET (useful for backfills)
```

`ct` / `mri` are cron-driven; `parse` exists for ops recovery and the parity harness.

## Per-stage design notes

### Boot query
Use SQL from `acquisition/on_boot_queries.js` verbatim. Map row to a strongly typed `SystemRow`. The `log_config` JSON column deserializes via `serde_json::from_value` into a typed `LogConfig`.

### File scan
`std::fs::File` + `BufReader` + `lines()`. Parse loop is CPU-bound and synchronous — wrap the per-system function in `tokio::task::spawn_blocking` if/when overlapping parse with PG insert from a previous system. v1 stays serial to match Node behavior.

### Regex
Compile both `re_v1` and `re_v2` once at startup as `OnceLock<Regex>`. Selection follows `file_config.parsers[0]` from the boot query.

### Cursor compare
Read the Redis-stored line as `String`; compare with `line == redis_line` byte-equal. CRLF handling is Open Decision #5 — if files are CRLF, apply `trim_end_matches('\r')` symmetrically on read and write.

### Datetime construction
Centralize in `time::host_datetime(host_date, host_time, tz) -> Result<DateTime<FixedOffset>>`. Policy swaps live in one place.

### Persist — `log.siemens_ct` / `log.siemens_mri`
Multi-row INSERT via `UNNEST` for v1, one round-trip. If backfills demand it, switch to binary `COPY` later.

### Persist — `alert.offline_hhm_conn` upsert
Short parameterized SQL — do **not** string-interpolate `sme` (the current Node code does; technically SQL-injectable, though `sme` is constrained server-side).

### Persist — `log.saved_files` (gzip)
Read file fully, gzip into `Vec<u8>`, INSERT with the buffer as `bytea`. `flate2` is standard.

### Redis cursor write
Last step. If it fails, **do not retry** — the row insert already committed; duplicate-cursor on next run is preferable to losing the rows. Log error and continue.

### CT scan-seconds extraction
Out of scope for v1. The `extract` call at end of `jobs/win_10/siemens_ct.js:202-203`. Port in v1.1.

## Containerization

Multi-stage Dockerfile producing a binary-only runtime image. The current Node pattern bind-mounts source at `/workspace` and runs `npm ci` + `npm run <job>`; the Rust app skips both — binary is baked into the image.

### Dockerfile structure

```
stage 1: builder
  FROM rust:1.<lts>-bookworm
  cargo-chef recipe layer       ← dep-only cache layer
  full source layer             ← only rebuilds on src changes
  cargo build --release --bin siemens_rpp

stage 2: runtime
  FROM debian:bookworm-slim     ← matches glibc; allows bash-lc parity
  apt: ca-certificates, tzdata, gosu
  COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
  COPY --from=builder /target/release/siemens_rpp /usr/local/bin/siemens_rpp
  WORKDIR /workspace
  ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
  CMD ["siemens_rpp", "--help"]
```

Key choices:
- `debian:bookworm-slim` runtime, not distroless — keeps bash for `bash -lc` parity if Open Decision #8 needs it.
- `tzdata` is mandatory insurance for any code path touching `/etc/localtime`.
- `gosu` + the verbatim `entrypoint.sh` from `data_acquisition/docker/entrypoint.sh` — `RUN_USER` env drops privileges identically.
- `cargo-chef` for dep-layer caching — single highest-ROI build tweak (~30s incremental vs ~5min cold).
- User UID setup mirrors the Node Dockerfile (`svc` / `jonathan-pope` / `matt-teixeira`) unless Open Decision #10 simplifies.

Final image size target: ~80–100 MB (vs `node:lts` ~400 MB, vs `data-acqu:staging` ~600 MB).

### Compose file shape

Mirrors `data_acquisition/docker-compose.yaml` so cron entries and operator habits transfer.

```yaml
x-common-env: &common_env
  env_file:
    - .env
  extra_hosts:
    - "host.docker.internal:host-gateway"
  environment:
    HOME: /tmp

x-common-mounts: &common_mounts
  working_dir: /workspace
  volumes:
    # NO source bind mount — binary baked in
    # NO node_modules cache — n/a
    - ${RUN_LOGS_DIR}
    - /opt/resources/acqu_files:/opt/resources/acqu_files:ro

services:
  app_tools:
    image: siemens-rpp:staging
    build:
      context: .
      dockerfile: docker/Dockerfile
      args:
        DOCKER_GID: ${DOCKER_GID}
        UID_0: ${UID_0}
        UID_1: ${UID_1}
        UID_2: ${UID_2}
    <<: [*common_env, *common_mounts]
    environment:
      RUN_USER: ${RUN_USER}
      REDIS_HOST: ${REDIS_HOST}
      REDIS_PORT: "${REDIS_PORT}"
    networks:
      - redis_net
      - pg_net

networks:
  redis_net:
    external: true
    name: redis-admin_redis_net
  pg_net:
    external: true
```

Differences from the Node siemens compose:
- Single `app_tools` service — no parallel `app` service (nothing useful without the binary).
- New mount root `/opt/resources/acqu_files:ro`.
- No `node_mod_cache` mount.
- No SSH key mount (Rust app doesn't shell out).
- No source bind mount — production image is immutable.

### `.env` contract

| Var | Status | Used by |
|---|---|---|
| `APP_NAME` | **new** (`siemens_rpp`) | log path, `util.app_run_logs` |
| `LOGGER` (dev/staging/prod) | reused | log filename + dev console |
| `RUN_ENV` | reused | log path selection |
| `RUN_USER` | reused | gosu drop |
| `REDIS_HOST`, `REDIS_PORT` | reused | redis cursor |
| `PG_*` | reused | tokio-postgres connection |
| `DOCKER_GID`, `UID_0/1/2` | reused | image build |
| `RUN_LOGS_DIR` | reused | logs bind-mount string |
| `ACQU_FILES_ROOT` | **new** (renamed from `DATA_STORE_DEV`) | file-store root = `/opt/resources/acqu_files`; name-only change (TD-015) |
| `NODE_MOD_CACHE_DEV` | **dropped** | n/a |

### Dev workflow without source bind-mount

Inner-loop dev uses native cargo on host (`cargo run -- ct` against `.env`, reach Redis/PG via `host.docker.internal` or network names). Optional `docker-compose.dev.yaml` override adds a `rust:1.x` service with bind-mounted source and `cargo watch` for repro-in-container cases. Production image stays binary-only.

### Operator commands

```sh
# First deploy / image rebuild
docker compose build app_tools

# Run a job (after Open Decision #8 lands on Option A)
docker compose run --rm app_tools siemens_rpp ct
docker compose run --rm app_tools siemens_rpp mri
```

## Behavior parity risks

In rough order of likelihood:

1. **CRLF in the cursor line.** Top risk. Node's `readline` with `crlfDelay: Infinity` strips `\r\n` and `\n` so the cached cursor has no line ending. Rust's `BufRead::lines()` strips `\n` but leaves trailing `\r` on CRLF files. Mitigation symmetrical on read/write per Open Decision #5.
2. **Bad-match Node bug** at `jobs/win_10/siemens_ct.js:84-107`. Latent crash on non-blank unmatched lines. Either the warn path is never hit in prod or the throw is silently swallowed. Recommend warn-and-skip in Rust + count instrumentation. Open Decision #6.
3. **`mapDataToSchema` NULL fields.** `domain_group`, `id_group`, `month`, `day`, `year` are NULL today because the regex doesn't capture them. Replicate exactly.
4. **Timezone default.** `time_zone_id` NULL → `America/New_York`, matching `processing/generateDateTimes.js:17`.
5. **DST ambiguous-hour policy.** Match luxon "earliest" per Open Decision #3.
6. **`note` JSON shape in `util.app_run_logs`.** Node stringifies the full array as one JSON text. Rust should produce same wire shape. Open Decision #4.

## Migration plan

| Phase | What | Exit criterion |
|---|---|---|
| 0 | Build shared crates: `rpp_log`, `rpp_db`, `rpp_redis`. Smoke-test by writing a "hello" run into `util.app_run_logs`. | One Rust-produced row visible alongside Node rows in `util.app_run_logs`. |
| 0.5 | Build the multi-stage Dockerfile and compose file. Confirm `docker compose run --rm app_tools siemens_rpp --help` exits 0. Confirm `/opt/run-logs/siemens_rpp/` is writable by `RUN_USER`. | "Hello" run produces both a file in `/opt/run-logs/siemens_rpp/` and a row in `util.app_run_logs` from inside the container. |
| 1 | Build `siemens_rpp parse` single-system subcommand. Wire against one dev SME in `--dry-run` mode. | Output rows match Node output rows for the same file, byte-equal except for `host_datetime` (see phase 2). |
| 2 | Datetime parity test: 1000 prod rows → both implementations → assert identical. | 100% match (or DST-ambiguity rows isolated and policy chosen). |
| 3 | Shadow run: cron the Rust binary in parallel with Node, writing to `log.siemens_ct_shadow` / `log.siemens_mri_shadow` (Open Decision #7). Same Redis namespace prefixed with `shadow.` to avoid clobbering the production cursor. ~1 week. | Row count + content diffs vs prod table are zero (or explainable). |
| 4 | Cut one SME over: exclude from Node boot query, include in Rust. Redis cursor for that SME owned by Rust. | One week stable. |
| 5 | Expand to remaining CT systems, then MRI. | All siemens_ct + siemens_mri SMEs on Rust. Node app stops being invoked for CT/MRI. |
| 6 | Deprecate `hhm_rpp_siemens` Node app entirely (siemens_cv already deprecated; win_7 either ported or moved to a `siemens_rpp legacy` subcommand). | Cron entries removed; Node app `npm run` scripts removed. |

## What's decided vs. what's open (handoff summary)

**Decided:**
- Scope: CT + MRI only; CV/IR dropped; win_7 deferred.
- Single Rust app at `~/apps/siemens_rpp/`, Cargo workspace.
- Shared crates `rpp_log` / `rpp_db` / `rpp_redis` live in this workspace for now, refactored to standalone when app #2 exists.
- Logging: reuse `util.app_run_logs` table verbatim, no Rust-specific log table.
- PG client: `tokio-postgres` + `deadpool-postgres`.
- Image: multi-stage Dockerfile, binary baked in, no source mount in prod.
- Ephemeral container model preserved (`docker compose run --rm app_tools …`).
- Cutover: shadow tables + parity harness + per-SME ramp.

**Resolved 2026-05-29 (were scaffold-blocking; see TD-015…TD-018):**
- #1 — File path: `/opt/resources/acqu_files/<system_id>/<file_name>`, root via `ACQU_FILES_ROOT`.
- #5 — CRLF: strip trailing `\r` symmetrically (emulate Node `readline`).
- #6 — Bad-match: warn-and-skip (documented divergence from Node's crash-and-wedge).
- #8 — Invocation: native subcommand (Option A).

**Open (can wait until scaffold time):**
- #2 — Date format strictness.
- #3 — DST ambiguous-hour policy (default: match luxon "earliest").
- #4 — `note` field byte-exact serialization vs typed.
- #7 — Shadow tables now vs migration time.
- #9 — Runtime base image: `debian:bookworm-slim` (default) vs distroless.
- #10 — User UID setup in image.

## Source-file references for context

Verified file:line citations used to build this plan (all relative to `~/apps/`):

- `hhm_rpp_siemens/index.js` — boot orchestration
- `hhm_rpp_siemens/jobs/index.js` — manufacturer router
- `hhm_rpp_siemens/jobs/win_10/index.js` — modality router
- `hhm_rpp_siemens/jobs/win_10/siemens_ct.js` — CT parser
- `hhm_rpp_siemens/jobs/win_10/siemens_mri.js` — MRI parser
- `hhm_rpp_siemens/parse/parsers.js` — `win_10_re` regexes
- `hhm_rpp_siemens/acquisition/Siemens_10.js` — `System` subclass, path construction
- `hhm_rpp_siemens/acquisition/on_boot_queries.js` — boot SQL
- `hhm_rpp_siemens/persist/pg-schemas.js` — `siemens_ct_mri` schema
- `hhm_rpp_siemens/utils/db/sql/pg-helpers_hhm.js` — column sets, target tables
- `hhm_rpp_siemens/utils/logger/log.js` — logging contract
- `hhm_rpp_siemens/utils/logger/enums.js` — type/tag enums
- `hhm_rpp_siemens/redis/redisHelpers.js` — cursor key format
- `hhm_rpp_siemens/redis/index.js` — Redis connection
- `hhm_rpp_siemens/processing/generateDateTimes.js` — datetime entry point
- `hhm_rpp_siemens/processing/dateTimeTemplate.js` — luxon call
- `hhm_rpp_siemens/tooling/dates.js` — `dt_now` (capture_datetime)
- `hhm_rpp_siemens/tooling/upsertHostDatatime.js` — `alert.offline_hhm_conn` upsert
- `hhm_rpp_siemens/tooling/gzip_file.js` — `log.saved_files` writer
- `hhm_rpp_siemens/docker-compose.yaml` — current compose pattern
- `data_acquisition/docker-compose.yaml` — sibling compose with image build
- `data_acquisition/docker/Dockerfile` — base image with gosu + UIDs
- `data_acquisition/docker/entrypoint.sh` — gosu drop, to be reused verbatim
- `data_acquisition/docs/run-notes.md` — current invocation pattern
