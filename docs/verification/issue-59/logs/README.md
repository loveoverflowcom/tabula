# Captured tool logs

Historical artifact notice: raw captures, generated receipts/logs and design exports
were removed from the source tree. Pinned links below use the pre-cleanup archive
[`80d9fdb9`](https://github.com/loveoverflowcom/tabula/tree/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/logs); those artifacts describe their original
source/build and do not establish current runtime acceptance. New output belongs in
ignored `verification/` directories or GitHub Actions Artifacts.


All captured tool output and the historical adaptation diff are preserved byte-for-byte
in [Historical raw-logs.zip](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/raw-logs.zip). [Historical log-index.json](https://github.com/loveoverflowcom/tabula/blob/80d9fdb96f18cd59fa85c9401b533d66bf04b5d7/docs/verification/issue-59/log-index.json) records every
original path, byte count and SHA256 digest. This keeps whitespace intrinsic to
Cargo diagnostics and unified-diff context out of the source diff check.

Extract from the repository root:

```sh
unzip docs/verification/issue-59/raw-logs.zip -d /tmp/tabula-59-evidence
```

Final authoritative output is `logs/tabula-59-final-core-check.log` and
`logs/tabula-59-final-focused.log`; earlier attempts are retained separately.
Exact checks and outcomes are listed in the issue ledger.
