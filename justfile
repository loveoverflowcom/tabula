# Human-facing task runner. The machine-facing one is `xtask` (pure Rust,
# cross-platform, testable). Anything CI depends on must live in xtask, not here —
# `just` is a convenience wrapper, never a source of truth. (doc 01 §1.4)

default:
    @just --list

# ---------------------------------------------------------------- development

# Start the local dependency stack: postgres, otel-collector + jaeger, minio.
# (doc 06 §3.1) — Phase 4; the compose file is a stub until then.
dev-deps:
    docker compose -f deploy/compose/dev.yml up -d

dev-deps-down:
    docker compose -f deploy/compose/dev.yml down

# Run the server against the local stack. Phase 4+.
server:
    cargo run -p tabula-server

# Leptos application shell on http://localhost:8080. Phase 5+.
web:
    cd apps/web && trunk serve

# Native Macroquad client. Phase 2+.
client *ARGS:
    cargo run -p tabula-game-client {{ARGS}}

# ------------------------------------------------------------------ the gate
# Authoritative portable local core gate (doc 01 §1.4). Runs the fast,
# deterministic checks: fmt, clippy, test, check-deps, check-no-game-ids,
# check-manifests, token freshness, check-no-raw-colors, and cargo deny.
#
# CI runs these same gates, plus the workspace feature matrix (`features`) and
# target-specific WASM compilation checks (`wasm`).
check:
    cargo xtask check

# Runs the local core gate plus workspace feature matrix checks.
check-all: check features

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

# I-1 / I-15: the deps.toml matrix, walked over resolved cargo metadata.
deps:
    cargo xtask check-deps

# I-9: no platform crate may name a game.
no-game-ids:
    cargo xtask check-no-game-ids

# game.toml must equal the compiled GameMetadata/GameCapabilities.
manifests:
    cargo xtask check-manifests

test:
    cargo nextest run --workspace

# Every crate must build with no features and with all features. (doc 01 §5.1)
features:
    cargo check --workspace --no-default-features
    cargo check --workspace --all-features

# No raw colors outside tabula-design. (doc 04 §8.1)
colors:
    cargo xtask check-no-raw-colors

# Design tokens generation check.
tokens-check:
    cargo xtask gen-tokens
    git diff --exit-code -- apps/web/style/tokens.css crates/tabula-design/src/generated.rs docs/ui/tokens.json

audit:
    cargo deny check

# ------------------------------------------------------------- determinism

# The highest-value test we have: bots play each other and every match is
# checked for determinism, projection safety, and termination. (doc 02 §11.3)
selfplay game matches="10000":
    cargo xtask selfplay {{game}} --matches {{matches}}

# Replay a golden or production .tbr; diagnostic mode reports evidence strength. (doc 05 §8.3)
replay file:
    cargo xtask replay {{file}}

# I-8 over the whole committed corpus. Nightly in CI, on demand here.
replay-all:
    cargo xtask replay --all

# ------------------------------------------------------------ verification
# Development-only, opt-in verification tools. They are deliberately outside
# `cargo xtask check` until real proof harnesses / a mutation budget justify CI.

verification-install:
    cargo install --locked kani-verifier --version 0.67.0
    cargo kani setup
    cargo install --locked cargo-nextest --version 0.9.143
    cargo install --locked cargo-mutants --version 27.1.0

# Proves the real logical-time arithmetic in tabula-core over its symbolic u64
# domains. Kani is opt-in and is not part of the normal workspace gate.
kani-core:
    cargo kani -p tabula-core


# Preview the mutation set for one named workspace package.
mutants-list package:
    cargo mutants --package {{package}} --list

# Run mutation testing for one named workspace package. `.cargo/mutants.toml`
# selects Nextest so this follows the repository's ordinary test runner policy.
mutants package:
    cargo mutants --package {{package}}

# --------------------------------------------------------------- generation
# All of these are committed outputs. CI fails if they are stale.

tokens:
    cargo xtask gen-tokens

protocol-vectors bump:
    cargo xtask gen-protocol-vectors --bump {{bump}}

pack game:
    cargo xtask pack-assets {{game}}

new-game slug *ARGS:
    cargo xtask new-game {{slug}} {{ARGS}}

# ---------------------------------------------------------------- database
# Phase 4+.

db-reset:
    cargo xtask db reset

db-migrate:
    cargo xtask db migrate

# Regenerate .sqlx/ so the workspace builds without a live database.
sqlx-prepare:
    cargo sqlx prepare --workspace -- --all-targets

# ------------------------------------------------------------------- builds

wasm-game:
    cargo build -p tabula-game-client --target wasm32-unknown-unknown --profile wasm-release
    cargo xtask stage-wasm-game

# Serve the staged Macroquad gameplay client in a local browser.
wasm-serve port="8000": wasm-game
    @echo "Serving Tabula gameplay client at http://localhost:{{port}}"
    python3 -m http.server {{port}} --directory target/tabula-web-game

server-release:
    cargo build -p tabula-server --profile release-server
