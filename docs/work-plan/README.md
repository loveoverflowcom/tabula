# Upcoming work

Numeric prefixes, when present, are the current recommended sequence and may
be renumbered; they are not permanent issue IDs. GitHub issues own acceptance
and the screen specifications own UI contracts.

This checkout handles **#59 only**, on the verified `develop` baseline
`7ffda7d00f17cc085f2c5169d68c05da13c23dab`. The verified-byte PNG/texture seam,
Sprite backend and local Tiles fixture/motion slice are implemented. Runtime
and acceptance evidence belongs in [the #59 ledger](../verification/issue-59/README.md).

The user's next task is **#60 in a separate chat**, after the #59 publication
handoff. Reuse [the measured fixture protocol](../perf/tiles-renderer-baseline.md)
and its pinned pack hashes, permitted view, accepted workload and motion options;
compare embedding and renderer cost separately. Native runtime and additional
browser/DPI evidence must retain their actual status, and cannot be inferred from
compilation or a different target. Do not implement the RFC experiment here.

The [#52 history/replay slice](backlog/issue-52-history-replay.md) remains gated.
#53's [board/modes](backlog/issue-53-board-modes.md),
[engine/resources](backlog/issue-53-engine-evidence.md) and
[Learn](backlog/issue-53-learn.md) follow their explicit prerequisites.
Passing existing baseline checks does not open these gates or close #52/#53.
