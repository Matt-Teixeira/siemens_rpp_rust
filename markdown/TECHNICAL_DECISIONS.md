# Technical Decisions

Durable, decided technical choices for `siemens_rpp`. **Open** (undecided) items
live in [siemens_rpp_plan.md](siemens_rpp_plan.md) ("Architecture decisions still
open"); this file records what is settled. Add a new TD entry when a decision is
made; update the existing one (with a dated note) if it changes.

---

## TD-001 — Port the Node app to Rust

**Decision:** Replace `hhm_rpp_siemens` (Node) with a Rust app for CT and MRI.

**Reason:** The hot path is per-line regex over large log files plus tz-aware
datetime construction — a strong Rust fit. A static binary also deploys to a much
smaller container than `node:lts` and removes the per-run `npm ci` cost.

**Tradeoffs:** Rewrite risk, mitigated by the parity harness and shadow-table
ramp.

---

## TD-002 — Data-contract parity is the v1 north star

**Decision:** Keep behavior bit-identical to Node where observable: PG tables and
column sets, Redis key/value, `util.app_run_logs` JSON shape, and datetime
formatting.

**Reason:** Downstream consumers (dashboards, alert/log tables) must not notice the
swap.

**Tradeoffs:** Some "better" designs are deferred behind explicit divergence
decisions (see TD-010).

---

## TD-003 — Postgres via `tokio-postgres` + `deadpool-postgres`

**Decision:** Use `tokio-postgres` with a `deadpool-postgres` pool.

**Reason:** `sqlx` compile-time checking would require a live DB in CI;
`tokio-postgres` is more pragmatic here. Bulk insert via `UNNEST` (see TD-009).

**Tradeoffs:** No compile-time query verification; mitigated by integration tests.

---

## TD-004 — Redis via the `redis` crate, simple connection preserved

**Decision:** Use the `redis` crate with the existing connection contract (host +
port, no auth, no TLS, no db selector). Key shape `${sme}.${file_name}`; value is
the file's first line stored raw.

**Reason:** Match the Node Redis contract exactly.

---

## TD-005 — Datetime via `chrono` + `chrono-tz`

**Decision:** Construct `host_datetime` through one centralized
`time::host_datetime(...)`. Source-verified details (`processing/generateDateTimes.js`,
`processing/dateTimeTemplate.js`, `tooling/dates.js`):

- The Node input string is `${hostDate}${hostTime}` — **concatenated with no
  separator** — parsed with luxon pattern `yyyy-MM-ddHH:mm:ss` (no space). The
  chrono equivalent is `"%Y-%m-%d%H:%M:%S"` (no space).
- Output is luxon `.toISO()`, e.g. `2026-05-29T15:00:00.000-04:00` — includes
  `.000` milliseconds and a `±HH:MM` offset. The chrono format must match:
  `"%Y-%m-%dT%H:%M:%S%.3f%:z"`.
- `capture_datetime` = `dt_now()` = `DateTime.now().setZone("America/New_York").toISO()`,
  computed once per run — same `.toISO()` shape.
- Timezone from `sites.time_zone_id`, falling back to `America/New_York` when NULL
  (`generateDateTimes.js:17`).

**Reason:** Byte-identical datetime output is a parity requirement; centralizing
keeps the format/timezone/DST policy in one place.

**Open sub-items (in the plan):** date-format strictness (#2 — see TD-015 note: the
win_10 regex allows single-digit month/day, so the date is **not** guaranteed
zero-padded, and the no-separator concatenation makes the date/time boundary
parse-sensitive) and DST ambiguous-hour policy (#3, default: match luxon
"earliest"). The datetime parity test gates cutover.

---

## TD-006 — Reuse `util.app_run_logs`; no Rust-specific log table

**Decision:** Reproduce the Node logging contract: an in-memory event array → one
end-of-run INSERT (verbose + warn/error) plus the file write to
`/opt/run-logs/${APP_NAME}/...`. Event `type`/`tag` stay the existing uppercase
strings. Encapsulated in the `rpp_log` crate; a `tracing` layer sits on top for dev
ergonomics but buffered events are authoritative.

**Reason:** Existing dashboards keep working unchanged.

**Open sub-item:** `note` byte-exact serialization vs typed serde shape (#4).

---

## TD-007 — Shared fleet crates in-workspace

**Decision:** `rpp_log`, `rpp_db`, `rpp_redis` live in this Cargo workspace for
now, designed to be lifted into a standalone workspace once a second Rust app
exists.

**Reason:** Reuse across the fleet without premature multi-repo overhead.

---

## TD-008 — Multi-stage Docker, binary-only runtime

**Decision:** Multi-stage build (`rust` builder with `cargo-chef` dep caching →
`debian:bookworm-slim` runtime with `ca-certificates`, `tzdata`, `gosu`). The
production image bakes the binary in — no source bind mount, no runtime package
install. Compose mirrors `data_acquisition`.

**Reason:** Small, immutable, fast incremental builds; `tzdata` insures any
`/etc/localtime` path; `gosu` + the reused entrypoint drop privileges via
`RUN_USER`.

**Open sub-items:** base image distroless vs slim (#9, default slim) and image user
UIDs (#10).

---

## TD-009 — Bulk insert via `UNNEST` for v1

**Decision:** Insert parsed rows with a multi-row `UNNEST` INSERT (one round-trip).

**Reason:** Simple, easy to evolve to `ON CONFLICT`. Revisit binary `COPY` only if
backfills demand it.

---

## TD-010 — Parameterize the `offline_hhm_conn` upsert (approved divergence)

**Decision:** Use parameterized SQL for the `alert.offline_hhm_conn` upsert rather
than string-interpolating `sme` as the Node code does.

**Reason:** Removes an injection surface. This is an intentional, documented
divergence from Node behavior; output rows are unchanged, so parity is preserved.

---

## TD-011 — Synchronous, serial parse loop for v1

**Decision:** `tokio` is used only because PG/Redis clients are async. The per-line
parse loop stays synchronous; systems are processed serially to match Node
ordering.

**Reason:** Parity and simplicity. Add `spawn_blocking`/overlap only behind a
measured need and a new decision.

---

## TD-012 — Redis cursor written last, never retried

**Decision:** Write the cursor as the final step of a system; on failure, log and
continue without retry.

**Reason:** Rows are already committed; a duplicate-cursor next run is preferable to
losing rows.

---

## TD-013 — v1 scope: CT + MRI only

**Decision:** Implement CT and MRI parsers only. `siemens_cv` is dropped
(deprecated); the `win_7` path is deferred; the CT scan-seconds sidecar is deferred
to v1.1.

**Reason:** Focus the port on the live, in-use paths.

---

## TD-014 — AI roles: Claude implements, Codex reviews

**Decision:** On this project Claude is the implementation tool and Codex is the
reviewer (swapped from the source scaffolding template).

**Reason:** Project preference for this port. See [FLOW.md](FLOW.md).

---

## TD-020 — Postgres connection requires TLS

**Decision:** The PG client must connect over TLS, loading the CA certificate. Use
`tokio-postgres` with a TLS connector — `postgres-native-tls` (native-tls/OpenSSL)
or `tokio-postgres-rustls` — wired into the `deadpool-postgres` pool. Plain `NoTls`
will **not** work against this server.

**Source evidence:** the live dev connection (from the Node `hhm_rpp_siemens` `.env`)
sets `PG_SSLMODE=require` and `PG_SSL_PATH=./hhm_rpp_siemens/pg_ssl.crt`; the sibling
`data_acquisition` ships `db/BaltimoreCyberTrustRoot.crt.pem` and `pg_ssl.crt`. So
the server enforces SSL and the client presents/verifies against a CA cert.

**Config:** expose `PG_SSLMODE` and `PG_SSL_PATH` (CA cert path) as config; the cert
file is mounted/copied into the image, never embedded in source. The exact
verify-full vs verify-ca vs require posture (hostname verification) should match what
the Node `pg`/`pg-promise` client does — confirm during Phase 0 when the pool is
built, and pin it here then.

**Reason:** Connectivity correctness; this was missing from the original crate-stack
row (which listed only `tokio-postgres` + `deadpool-postgres`). Folded into the plan's
crate stack.

---

## TD-015 — File path shape (resolves Open Decision #1)

**Decision:** `{root}/{system_id}/{file_name}`, where `root = DATA_STORE_DEV`.

**Source evidence:**
- `acquisition/Siemens_10.js:12,15` —
  `data_acqu_path = process.env.DATA_STORE_DEV` and
  `complete_file_path = ${data_acqu_path}/${sysConfigData.id}/${file_config.file_name}`.
- `jobs/win_10/index.js:58` (the gzip path) uses the same
  `${data_acqu_path}/${sysConfigData.id}/${log_config.file_name}`.
- `hhm_rpp_siemens/.env:26` — `DATA_STORE_DEV=/opt/resources/acqu_files`.

So the effective path is `/opt/resources/acqu_files/<system_id>/<file_name>`.
This is the **first** of the three candidates in the plan. `log_config.dir_name`
is **not** used in the siemens win_10 path (it's a magnet/RMMU concept in
`data_acquisition/relocate_files/rsync_local.js`).

**Correction to the plan:** the Node app's `DATA_STORE_DEV` was **not** "dropped" —
its value already *is* `/opt/resources/acqu_files`. The Rust app needs that root as
config.

**Rename (decided 2026-05-29):** the Rust app exposes this root as
**`ACQU_FILES_ROOT`** instead of carrying over the misleading `DATA_STORE_DEV`
name. **Only the variable name changes; the value stays `/opt/resources/acqu_files`**
and the read-only file-store mount is unchanged. This is a deliberate, contained
`.env` divergence from the Node app (the Rust app has its own fresh `.env`, so
nothing downstream depends on the old name). The `PathBuilder` keyed on
`(manufacturer, modality)` is still useful for GE/Philips later, but for siemens v1
the rule is the single shape above, rooted at `ACQU_FILES_ROOT`.

**Tradeoffs:** A one-line `.env`-name difference from Node, accepted for clarity;
no behavioral/path difference. The CLI `--file <path>` override remains for
one-offs.

---

## TD-016 — CRLF cursor handling (resolves Open Decision #5)

**Decision:** Strip a trailing `\r` from each line before compare/store, applied
**symmetrically** on read and write — emulating Node `readline`.

**Source evidence:** `acquisition/Siemens_10.js:90-93` reads via
`readline.createInterface({ input: createReadStream(...), crlfDelay: Infinity })`,
which strips both `\n` and `\r\n`, so lines carry **no** trailing `\r`. The cursor
is stored from such a line (`update_redis_line(first_line)`) and compared against
such a line (`line == System.redis_line`), so both sides are already `\r`-free.

Rust `BufRead::lines()` strips `\n` but leaves `\r` on CRLF files, so the Rust port
must trim the trailing `\r` (e.g. `line.strip_suffix('\r')`) on both the scan side
and the value written to Redis. Store the cursor value otherwise raw — no other
trim/normalize/re-encode.

**Note:** This was flagged as the top parity risk. With symmetric trimming it is
neutralized; the parity harness should still include a CRLF capture.

---

## TD-017 — Bad-match behavior: warn-and-skip (resolves Open Decision #6, divergence)

**Decision (ratified 2026-05-29):** On a non-blank line that fails the regex, log a
WARN and **skip the line** (`continue`); do not abort the system. This is an
intentional, documented divergence from Node and is the chosen behavior for v1.

**Line classification (source-exact).** For each scanned line, after the
cursor-hit check:
1. Try the active regex (`re_v1`/`re_v2` per `parsers[0]`).
2. If it matches → build the row.
3. If it does **not** match, apply Node's blank test
   `blankLineTest = /^[ \t\n]*$/` (`tooling/regExHelpers.js:30-33`) — i.e. the line
   is empty or only spaces/tabs/newline:
   - **blank → skip** (`continue`). *This already matches Node.*
   - **non-blank → "Bad Match"**: log a WARN with the offending line, increment a
     skip counter, and `continue`. *This is the divergence.*

**Source evidence (what Node actually does on the non-blank branch):** In
`jobs/win_10/siemens_ct.js:84-107` (and the MRI twin), when `matches === null` and
the line is not blank, it logs the WARN and then **falls through** to
`matches.groups.system_id = System.sme` — dereferencing `null.groups`, which
**throws**. The throw is caught by the system-level `try/catch` (~line 206), logged
as ERROR, and the function returns. Consequences: rows collected so far are **not**
inserted (the throw precedes the insert), and the Redis cursor is **not** advanced —
so that system reprocesses the same data every run until the bad line ages out. A
single malformed line silently wedges that system.

**Why diverge:** Replicating a crash that drops data and wedges the cursor has no
upside, and it conflicts with [IMPLEMENTATION_RULES.md](IMPLEMENTATION_RULES.md)
(no panics on runtime paths). Warn-and-skip is the clearly-intended behavior.

**Parity impact & required check:** Identical to Node on clean data (no bad
matches) and on blank lines. They differ **only** on non-blank malformed lines:
Node aborts+wedges; Rust skips+continues+counts. The Phase 1/2 parity harness
**must** report the bad-match count on the sampled prod captures; if it is always
zero, the divergence is unobservable in practice. The WARN note shape stays
parity-faithful (`message: "This is not a blank new line - Bad Match"`, plus the
`line`) so existing log consumers see the same event on the lines Node would have
warned on.

---

## TD-018 — Invocation: native subcommand (resolves Open Decision #8, Option A)

**Decision:** cron invokes the binary directly through the compose service:

```
cd /home/prod/siemens_rpp_rust && docker compose run --rm app_tools siemens_rpp ct
cd /home/prod/siemens_rpp_rust && docker compose run --rm app_tools siemens_rpp mri
```

No `bash -lc "npm run …"` indirection, no npm shim.

**Source evidence:** the current prod cron (`data_acquisition/prod_cron.txt:62-63`)
runs `cd /home/prod/hhm_rpp_siemens && npm run siemens_ct|mri`, where the npm
scripts are `node index.js SIEMENS_CT|SIEMENS_MRI` (`hhm_rpp_siemens/package.json`).
The cutover changes exactly those two lines to the new directory + native
subcommand.

**Tradeoffs:** ~2 cron lines change (plus the directory name). Cleaner than
carrying an npm shim solely for command-string parity.

---

## TD-019 — Repo directory vs application name

**Decision:** The repository directory is `siemens_rpp_rust` (disambiguates the
Rust port from the Node `hhm_rpp_siemens` it replaces). The **binary, crate, Docker
image, and `APP_NAME` remain `siemens_rpp`** — `APP_NAME` feeds the
`util.app_run_logs` rows and the `/opt/run-logs/${APP_NAME}/` path, so it is part
of the data contract and must not gain a `_rust` suffix.

**Reason:** Folder clarity without breaking the log/run-path contract. The plan's
prose uses `~/apps/siemens_rpp` for the app; read that as "the app named
`siemens_rpp`, living in the `siemens_rpp_rust` directory."
