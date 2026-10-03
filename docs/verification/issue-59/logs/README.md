# Captured tool logs

All captured tool output and the historical adaptation diff are preserved byte-for-byte
in [raw-logs.zip](../raw-logs.zip). [log-index.json](../log-index.json) records every
original path, byte count and SHA256 digest. This keeps whitespace intrinsic to
Cargo diagnostics and unified-diff context out of the source diff check.

Extract from the repository root:

```sh
unzip docs/verification/issue-59/raw-logs.zip -d /tmp/tabula-59-evidence
```

Final authoritative output is `logs/tabula-59-final-core-check.log` and
`logs/tabula-59-final-focused.log`; earlier attempts are retained separately.
Exact checks and outcomes are listed in the issue ledger.
