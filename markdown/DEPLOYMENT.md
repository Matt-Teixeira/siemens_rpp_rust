# siemens_rpp Deployment Guide

## Deployment Philosophy

`siemens_rpp` is a backend batch job, not a service or UI. It is deployed as a
small, immutable Docker image and invoked **ephemerally** per modality on a
schedule, mirroring the `data_acquisition` app's runtime model:

```bash
docker compose run --rm app_tools siemens_rpp ct
docker compose run --rm app_tools siemens_rpp mri
```

The binary is baked into the image — there is no source bind mount and no
`npm ci`/package install at runtime. See
[siemens_rpp_plan.md](siemens_rpp_plan.md) ("Containerization") for the full
rationale and [TECHNICAL_DECISIONS.md](TECHNICAL_DECISIONS.md) TD-008.

---

# Image

Multi-stage build:

```
stage 1 — builder
  FROM rust:1.<lts>-bookworm
  cargo-chef recipe layer      # dep-only cache layer
  full source layer            # rebuilds only on src changes
  cargo build --release --bin siemens_rpp

stage 2 — runtime
  FROM debian:bookworm-slim    # matches glibc; allows bash -lc parity if needed
  apt: ca-certificates, tzdata, gosu
  COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
  COPY --from=builder /target/release/siemens_rpp /usr/local/bin/siemens_rpp
  WORKDIR /workspace
  ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
  CMD ["siemens_rpp", "--help"]
```

Key choices:
- `debian:bookworm-slim` runtime (not distroless) — keeps `bash` for `bash -lc`
  parity if Open Decision #8 requires it (#9 default: slim).
- `tzdata` is mandatory for any code path touching `/etc/localtime`.
- `gosu` + the verbatim `entrypoint.sh` from `data_acquisition/docker/` drops
  privileges via `RUN_USER`.
- `cargo-chef` for dep-layer caching (~30s incremental vs ~5min cold).
- Image user UIDs follow Open Decision #10.

Target image size: ~80–100 MB (vs `node:lts` ~400 MB).

---

# Compose

Mirrors `data_acquisition/docker-compose.yaml` so cron entries and operator habits
transfer. Differences from the Node siemens compose:

- single `app_tools` service (no parallel `app` service)
- new read-only mount `/opt/resources/acqu_files:ro`
- no `node_mod_cache` mount, no SSH key mount, no source bind mount
- external `redis_net` / `pg_net` networks

See the plan for the full compose snippet.

---

# `.env` Contract

| Var | Status | Used by |
| --- | ------ | ------- |
| `APP_NAME` (`siemens_rpp`) | **new** | log path, `util.app_run_logs` |
| `LOGGER` (dev/staging/prod) | reused | log filename + dev console |
| `RUN_ENV` | reused | log path selection |
| `RUN_USER` | reused | gosu drop |
| `REDIS_HOST`, `REDIS_PORT` | reused | redis cursor |
| `PG_*` | reused | tokio-postgres connection |
| `DOCKER_GID`, `UID_0/1/2` | reused | image build |
| `RUN_LOGS_DIR` | reused | logs bind-mount string |
| `ACQU_FILES_ROOT` | **new** (renamed from `DATA_STORE_DEV`) | file-store root = `/opt/resources/acqu_files`; name-only change (TD-015) |
| `NODE_MOD_CACHE_DEV` | **dropped** | n/a (binary baked in) |

Secrets live only in `.env` / the container environment — never in source or the
image.

---

# Operator Commands

Invocation is the native subcommand form (Open Decision #8 → Option A; TD-018).
The prod cron lines that today read
`cd /home/prod/hhm_rpp_siemens && npm run siemens_ct|mri`
(`data_acquisition/prod_cron.txt:62-63`) become:

```bash
cd /home/prod/siemens_rpp_rust && docker compose run --rm app_tools siemens_rpp ct
cd /home/prod/siemens_rpp_rust && docker compose run --rm app_tools siemens_rpp mri
```

```bash
# Build / rebuild the image
docker compose build app_tools

# Run a job
docker compose run --rm app_tools siemens_rpp ct
docker compose run --rm app_tools siemens_rpp mri

# One-off / recovery against a single system
docker compose run --rm app_tools siemens_rpp parse \
  --system-id SMExxxxx --modality ct \
  --file /opt/resources/acqu_files/.../EvtApplication_Today.txt \
  [--dry-run] [--no-cursor-update]
```

---

# Local Dev (no source mount in prod)

Inner-loop dev uses native cargo on the host (`cargo run -- ct` against `.env`,
reaching Redis/PG via `host.docker.internal` or the docker network names). An
optional `docker-compose.dev.yaml` override can add a `rust:1.x` service with a
bind-mounted source and `cargo watch` for in-container reproduction. The production
image stays binary-only.

---

# Release Process

1. Bump the version in the workspace `Cargo.toml` (SemVer: `0.1.0`, `0.2.0`,
   `1.0.0`).
2. Run the gate: `cargo build`, `cargo test`,
   `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
   plus `cargo audit` / `cargo deny`.
3. Build the image: `docker compose build app_tools`.
4. Smoke test: `docker compose run --rm app_tools siemens_rpp --help`; confirm a
   "hello" run writes both a file under `/opt/run-logs/siemens_rpp/` and a row in
   `util.app_run_logs`.
5. Roll out per the plan's Migration plan (shadow → per-SME cutover), updating cron
   to point at the new image/invocation.

---

# Cutover & Cron

Cutover is staged, not big-bang (see the plan's Migration plan and
[PROMPTS.md](PROMPTS.md)):

- **Shadow (Phase 3):** cron the binary in parallel with Node, writing to
  `*_shadow` tables and a `shadow.`-prefixed Redis namespace so the production
  cursor is untouched.
- **Per-SME ramp (Phases 4–5):** move systems off the Node boot query and onto the
  Rust invocation one (then many) at a time; the Redis cursor for a migrated SME is
  owned by Rust.
- **Deprecate (Phase 6):** remove the Node cron entries and `npm run` scripts.

---

# Explicit Non-Goals (v1)

- No Heroku / hosted API (this is a batch job, not a server).
- No Kubernetes / orchestration beyond the existing compose + cron model.
- No UI, installer, or desktop artifact.
- No multi-manufacturer deployment (GE/Philips remain separate apps for now).
