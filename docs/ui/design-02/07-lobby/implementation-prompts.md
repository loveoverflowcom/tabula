## Prompt PR A — spec lobby state machine
```text
Pin develop HEAD, read AGENTS.md / doc 00 / doc 03 §14–15/doc 04 / doc 05 / doc 07 và tabula-lobby/web scaffolds. Dùng pack 07-lobby viết spec cho màn 17–19 trong docs/ui/screens/: routes/data/action/permissions, room-vs-match model, seat/ready/settings/start state machine, queue cancel-vs-found race, handoff loader/recovery. Reuse foundation controls và discovery config, server capability authority. Đánh dấu Phase 5 dependencies và fake prototype states. Không implement matchmaker/backend/networking để làm đẹp UI hoặc crossphase gate.
```

## Prompt PR B — room slice sau gate
```text
Sau Phase 4 exit và typed lobby/server contracts được chứng minh, implement browser→join→ready→start một room slice bằng shell foundation. Read AGENTS.md / doc 00 / doc 03 / doc 04/spec 07; ready/seat/start do authority quyết định, settings-change invalidation và duplicate start xử lý idempotent. Không shell game-id branching, không mutate match State; use GameCapabilities. Handoff separate document theo ADR-011 with compatibility and failure return path. Test full / private / started / disabled, ownership/seat permissions, disconnect and concurrent settings/start. Run targeted integration/UI gates+just check, distinguish doubles from real two-client proof.
```

## Prompt PR C — queue/handoff sau room slice
```text
Chỉ khi actual queue/match launcher/client-session adapter sẵn sàng, implement enqueue/cancel/match-found/give-up UI với typed states và elapsed time thật. Read AGENTS.md / doc 00 / doc 03 / doc 04 / doc 05/spec 07. Resolve cancel-vs-found and duplicate notifications qua server contract; không fake matched timer, không client tự cấp seat. Prefetch/loader byte progress chỉ khi assets delivery thật có; cancel/failure/back không leak join token hoặc bỏ orphan queue state. Test cancel trước/sau match, late/duplicate event, reconnect/session expiry và compatibility mismatch. Không rewrite matcher hoặc promise SLA từ mock.
```

