# Local/dev backend: issue #110 PR01

[ADR-0047](adr/0047-local-dev-backend-lifecycle.md) opens these explicit service
commands. Default binaries still refuse startup. This is a local development
composition, with existing direct matches and account/social authority; Room,
ready/start, result/history and rematch follow in PR02/PR03.

Use the repository toolchain, PostgreSQL 16, a configured local Kanidm provider,
Caddy and the normal web/WASM tools. Kanidm owns credential enrollment/MFA and
an operator-configured confidential `tabula` client with `openid`, S256 PKCE and
the exact callback `https://app.localhost:8444/api/v1/auth/oidc/callback`. This
runbook neither provisions a hosted provider nor changes its credential policy.

1. Start your development PostgreSQL (the repository provides
   `docker compose -f deploy/compose/dev.yml up -d postgres`). Use a database
   reserved for development, with no production data. Export its connection URL
   as both `TABULA_AUTH_DATABASE_URL` and `TABULA_SERVER_DATABASE_URL`.
2. Copy `deploy/local-dev/auth.toml` and `server.toml` to private local files.
   Set your configured provider/client and optional private CA PEM path in auth;
   keep both `browser_origin` values identical. Export the operator-supplied
   client secret as `TABULA_AUTH_CLIENT_SECRET`. Unknown keys and invalid types
   fail closed without printing their values.
3. Generate **one distinct CSRF signing secret** for this local installation
   using `python3 -c 'import secrets; print(secrets.token_urlsafe(32))'` in your
   private operator environment. Set the same value in `TABULA_AUTH_CSRF_KEY`
   and `TABULA_SERVER_CSRF_KEY`; keep it stable across service restarts. Never
   use a session credential as this key or include it in browser assets/logs.
4. Apply schema explicitly once, then use check-only startup:

   ```sh
   TABULA_AUTH_SCHEMA_POLICY=apply cargo run -p tabula-auth --features local-dev -- enrollment-enable --config /private/auth.toml
   cargo run -p tabula-auth --features local-dev -- serve --config /private/auth.toml
   cargo run -p tabula-server --features local-dev -- serve --config /private/server.toml
   ```

   The first command explicitly enables Tabula account enrollment for verified
   provider identities; it does not create provider credentials. Use
   `enrollment-disable` to close it. `schema_policy = "check"` never migrates.
   Absent/stale/unknown history, missing authority objects or a failed database
   connection refuse startup. Applying the combined existing schemas preserves
   all reviewed migration versions and data; it does not reset a database.
5. Build both independent browser documents:

   ```sh
   (cd apps/web && TABULA_PLAY_BASE=/play trunk build --release --cargo-profile wasm-release --features online,account-social)
   cargo build -p tabula-game-client --no-default-features --features web,online --target wasm32-unknown-unknown --profile wasm-release
   cargo xtask stage-local-play
   caddy run --config deploy/local-dev/Caddyfile --adapter caddyfile
   ```

   Configure trust for the local TLS edge and provider in your development
   browser. The service accepts the configured HTTPS origin; plain HTTP Trunk
   proxy URLs are not a cookie/login alternative. The supplied Caddyfile routes
   context, login/enrollment and session mutations to auth, and profile/social/
   match APIs to server. Signed-out context must stay with the auth process's
   pending-login map; both processes verify authenticated CSRF with the shared
   key and fresh PostgreSQL authority. Only server owns the social socket/hub.
6. Use two independent browser profiles at `https://app.localhost:8444`.
   Register each verified provider identity, then explicitly Login. Select
   Chess with no clock, Create, copy the join code to the second profile, Join,
   and enter the separate gameplay document. A short complete game is
   `f2-f3`, `e7-e5`, `g2-g4`, `d8-h4`: both boards report Black's checkmate.

`GET http://127.0.0.1:3001/healthz` and `:3002/healthz` report process liveness.
Each `/readyz` probes the actual combined database history/schema within five
seconds; the gameplay listener opens only after a supported registry is loaded.
Auth startup verifies provider discovery/keys; its readiness is not a live
provider-uptime claim. Health responses contain no database/provider detail.

Send SIGTERM/SIGINT to stop either process. The configured deadline (1–30 seconds,
15 in the sample) bounds HTTP, social stream and actor cleanup. A complete drain
exits successfully; a timeout/failure exits unsuccessfully and explicitly requires
durable recovery. Restart with the same database and configuration, then allow
clients to reacquire context/grants and the same server-owned seats. A timeout is
not proof an uncertain command failed; only the durable receipt resolves it.

Capacities are operator-visible: request/work 1–64, resident match owners 1–128,
service pool connections 2–40 and **historical admission records** 1–128 (sample16).
The lifetime count includes expired/completed direct-match admissions. Its
`room_capacity_exhausted` response is explicit; reclaim and reuse belong to PR02.
Do not delete journals/receipts or increase constants to manufacture capacity.
There is also an existing four-uncompleted-admissions per-user bound, 64 durable
admission receipts per session, 64 account enrollment capacity, and the current
bounded recovery/receipt policies. These are local/dev limits, not a scale claim.
Detached PostgreSQL owner backends consume connections outside the pool: reserve
at least both pool maxima plus the configured lifetime match capacity and admin
headroom (the sample uses 8+8+16 backends before headroom).

The [PR01 evidence ledger](verification/issue-110-pr01/README.md) distinguishes
real execution from source review and records the baseline continuity result.
