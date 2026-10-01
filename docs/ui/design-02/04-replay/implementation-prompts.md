## Prompt PR A — spec và replay boundary
```text
Pin develop HEAD loveoverflowcom/tabula, đọc AGENTS.md / doc 00 / doc 04 / doc 05 / doc 07, protocol/storage scaffolds và pack 04-replay. Viết spec cho màn 09–11 trong docs/ui/screens/: result ownership+handoff, history row/data mapping, generic replay controls/cursor/state timeline, permissions/rules-version/resource and failure matrix. Map source accepted inputs/snapshots authority; đánh dấu scrub runtime Phase 9 và persistence/protocol Phase 4 dependencies. Tách Xiangqi engine/tutor extension sang nhóm UI 05 (Xiangqi), không duplicate controller. Không implement fake history service hoặc AI replay reconstruction.
```

## Prompt PR B — một replay slice khi gate mở
```text
Khi persistence/protocol và replay-viewer phase được chứng minh, implement một supported game's history→result→read-only replay slice. Pin HEAD, đọc AGENTS.md / doc 00 / doc 04 / doc 05 và spec 04. Reconstruct đúng recorded rules version/accepted inputs, projection theo viewer; seek và snapshot selection dùng existing replay contract. UI chỉ nhận permitted view, timeline và structured errors; không canonical secret payload, không generate AI khi replay. Thử cursor 0 / end, repeated seek, incompatible rules/pack, malformed/oversize data và forbidden viewer; compare reconstruction/hash với authority fixture. Run targeted replay+UI tests và repo gate; report engine-free scope.
```

