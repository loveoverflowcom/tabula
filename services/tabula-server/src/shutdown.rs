//! Production deploy/WS drain remains gated; doc 06 §11.3.
//! ADR-0047's local/dev HTTP/social/actor deadline lives in `local_dev::serve`.
//! TODO(phase 4): prove fleet rolling-deploy drain and production connection handoff.
