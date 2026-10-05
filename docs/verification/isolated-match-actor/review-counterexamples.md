# Retained review counterexamples

## Process clock mistaken for match logical time

A new match initializes at logical zero. Using an already-running process's
monotonic value directly charged a fresh clocked game 600000ms on its first move.
The actor now captures an anchor at spawn, subtracts it with checked/clamped
elapsed semantics and never rewinds recorded time. The actual approved clocked
game regression starts its clock at600000, moves at600010, records10 and leaves
the player990ms. A separate fake-clock test records[0,10,10,30] across rewind.

## Private action exposed through operation-scope capacity

Lazy scope allocation let an owner occupy the sole scope with a private input;
an otherwise identical observer probe changed from RuleRejected to Busy although
its projected state and event stream were unchanged. Scopes are now reserved at
successful authorized attach. Command receipt storage changes no shared admission
capacity; full capacity refuses a new attachment without evicting a watermark.
The retained test compares admission/probes with and without the private input.

These are fixed offline contract defects, not production traffic-shaping or
durable revocation/delivery proofs. Shared execution latency and mailbox-load
timing remain explicitly outside the semantic stream/count/gap privacy claim.
