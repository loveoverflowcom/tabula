//! PHASE 4 — graceful drain; doc 06 §11.3.
//!
//! TODO(phase 4): on SIGTERM stop new attaches, snapshot/flush live matches,
//! send Draining and close 4411 within the bounded drain deadline; lazy rehydrate
//! on the next attach. Prove zero lost committed inputs with integration evidence.
