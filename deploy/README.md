# deploy/

Deployment artifacts. **Phase 4** for compose and systemd; **Stage 2+** for
terraform.

```text
compose/     local dev dependencies: postgres, otel-collector + jaeger, minio
systemd/     Stage 0-1 production units
terraform/   optional, Stage 2+. Empty on purpose — see below.
```

## The staging model (doc 06 §12)

| | Stage 0 | Stage 1 | Stage 2 | Stage 3+ |
|---|---|---|---|---|
| Indicative CCU | ≤ 100 | ≤ 1,000 | ≤ 25,000 | 100,000+ |
| Topology | 1 host, all-in-one | app host + DB host | gateways + workers | + shards, + regions |
| Processes | 1 | 1 | N + M | per region |
| Redis | no | no | yes | yes |
| Deployment | **systemd** | **systemd** | containers + rolling | per region |
| Kubernetes | no | no | only if the team has grown **and** containers-on-VMs is genuinely the bottleneck | maybe |
| Expected bottleneck | nothing | DB commit latency | connection memory; DB writes | RTT |

Each transition has a **measured trigger** in doc 06 §1.1. Moving early is not
prudence; it is paying an operational tax daily for a benefit you do not yet
receive (ADR-020).

## Stage 0 production, in full

```text
One VPS (4 vCPU, 8 GB) running:
   ├── caddy            TLS, HTTP/2, static files for apps/web, proxy to /ws
   ├── tabula-server    systemd unit, one process
   └── postgres 16      same host, dedicated data volume,
                        daily base backup + WAL archiving
Assets on a CDN (Cloudflare/Bunny) in front of an object-storage bucket.
Backups: pgBackRest or wal-g to object storage.
         RESTORE TESTED MONTHLY — an untested backup is not a backup.
```

Roughly $25–50/month. That is the correct amount to spend before there are
players.

## Stage 0 exit criteria (doc 06 §3.4)

```text
[ ] load test sustains 500 CCU with p95 ack < 60 ms on the production host class
[ ] a deploy causes ZERO lost matches (the drain path is verified, not assumed)
[ ] restore-from-backup rehearsed end to end
[ ] dashboards + alerts live
```

## Deploy sequence (doc 06 §11.3)

```text
1. new instance starts, passes /readyz (DB reachable, registry loaded,
   migrations current)
2. LB sends new connections to the new instance
3. old instance: SIGTERM → 15 s drain
     stop accepting attach → snapshot every live match → flush event batches
     → Draining{retry_after_ms: 250} → Close(4411)
4. clients reconnect IMMEDIATELY on 4411 (no backoff) and land on the new one
5. matches rehydrate lazily on attach
```

`TimeoutStopSec` in the systemd unit must exceed the 15 s drain deadline, or
systemd kills the process mid-drain and step 3 becomes a lie.

## Migrations gate the deploy

**Additive only** (doc 06 §11.2). Migration *N* must not break release *N-1*, or
rollback stops being possible exactly when it is needed. CI enforces this before
the artifact is promoted.

## Why `terraform/` is empty

It is Stage 2+ and conditional. Doc 09 §2 lists the deployment platform as
replaceable behind "one binary + Postgres, no platform coupling", with a cost of
"a Terraform/systemd change". Writing it now would create coupling to a topology
we have not measured a need for.

Keep it that way until doc 06 §1.1's trigger for Stage 2 has actually fired.
