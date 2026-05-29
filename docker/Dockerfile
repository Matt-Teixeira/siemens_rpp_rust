# syntax=docker/dockerfile:1
#
# Multi-stage build for siemens_rpp. The binary is baked into a small runtime
# image (no source bind mount, no package install at run time). See
# markdown/DEPLOYMENT.md and TD-008.

# ---------------------------------------------------------------------------
# Stage 1 — builder. cargo-chef caches the dependency layer so only src changes
# trigger a full recompile (~30s incremental vs ~5min cold).
# ---------------------------------------------------------------------------
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
# Build & cache dependencies from the recipe only.
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
# Now copy the full source and build just our crates.
COPY . .
RUN cargo build --release --bin siemens_rpp

# ---------------------------------------------------------------------------
# Stage 2 — runtime. debian:bookworm-slim (matches builder glibc), plus the
# system bits the app needs: TLS roots, tzdata (any /etc/localtime path), and
# gosu for the privilege-dropping entrypoint.
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

ARG DOCKER_GID
ARG UID_0
ARG UID_1
ARG UID_2

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates \
      tzdata \
      gosu \
 && rm -rf /var/lib/apt/lists/*

# Mirror the fleet user/group setup so host-mounted run-logs get sane ownership.
RUN groupadd -g ${DOCKER_GID} docker \
 && useradd -u ${UID_0} -g docker -m -d /home/svc -s /bin/bash svc \
 && useradd -u ${UID_1} -g docker -m -d /home/jonathan-pope -s /bin/bash jonathan-pope \
 && useradd -u ${UID_2} -g docker -m -d /home/matt-teixeira -s /bin/bash matt-teixeira

# Cooperative umask for all shells (matches data_acquisition).
RUN printf 'umask ${UMASK:-0002}\n' > /etc/profile.d/umask.sh \
 && chmod 644 /etc/profile.d/umask.sh

COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod 0755 /usr/local/bin/entrypoint.sh

COPY --from=builder /build/target/release/siemens_rpp /usr/local/bin/siemens_rpp

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["siemens_rpp", "--help"]
