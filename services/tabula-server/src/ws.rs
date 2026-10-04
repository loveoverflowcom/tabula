//! PHASE 4 — WebSocket boundary; doc 03 §3/§7 and ADR-0031.
//!
//! TODO(phase 4): authenticate HTTP upgrade before 101: browser cookie + exact
//! trusted Origin, or native bearer without cookies; reject mixed/duplicate input.
//! Hello carries compatibility metadata only. Decode the envelope, keep game
//! bytes opaque until registry dispatch, then enforce current match authority.
//! TODO(phase 4): bound frames, queues, mailboxes, rates and first-Hello deadline;
//! split reader/writer tasks, add heartbeat/slow-consumer handling and resume.
//! Session expiry/revocation must close sockets and fence queued private output.
