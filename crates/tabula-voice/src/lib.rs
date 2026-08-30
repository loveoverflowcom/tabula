//! # `tabula-voice` — voice as a separate plane
//!
//! > ## PHASE 8
//!
//! ADR-016: the **separation is locked**; the **provider is an experiment**
//! decided by measurement in Phase 8.
//!
//! Media traffic must never share the game WebSocket's ordering or backpressure
//! characteristics. A 20-person werewolf voice room saturating the socket that
//! carries votes would be a self-inflicted outage.
//!
//! ## We do not write an SFU
//!
//! > Media engineering is a company, not a feature. — doc 00 §12
//!
//! Mesh is fine for ≤4 participants; werewolf needs 6–20, which needs an SFU.
//! Buy managed, or self-host proven software (`LiveKit` is the reference
//! candidate). Both sit behind the same trait, and the Phase 8 exit criterion is
//! literally "provider swap demonstrated by running the suite against both
//! adapters".
//!
//! ## The trait (doc 03 §17)
//!
//! ```rust,ignore
//! pub trait VoiceService: Send + Sync {
//!     async fn ensure_rooms(&self, m: MatchId, scopes: &VoiceScopes) -> Result<(), VoiceError>;
//!     async fn join(&self, m: MatchId, room: &str, p: Participant, perms: VoicePerms)
//!         -> Result<JoinGrant, VoiceError>;
//!     async fn leave(&self, m: MatchId, room: &str, p: ParticipantId) -> Result<(), VoiceError>;
//!     async fn set_mute(&self, m: MatchId, p: ParticipantId, muted: bool) -> Result<(), VoiceError>;
//!     async fn stats(&self, m: MatchId) -> Result<VoiceStats, VoiceError>;
//!     async fn teardown(&self, m: MatchId) -> Result<(), VoiceError>;
//! }
//!
//! pub struct JoinGrant { pub url: String, pub token: String, pub ice_servers: Vec<IceServer> }
//! ```
//!
//! ## Ownership split — the same split as everything else
//!
//! The **game** decides which voice channels exist per phase and who is in them,
//! via `Effect::SetVoiceScopes`. The **platform** does signaling, room lifecycle,
//! TURN/SFU, and mute enforcement. The game never touches a socket or an SFU.
//!
//! Werewolf's night phase is the whole reason this exists:
//!
//! ```rust,ignore
//! effects.push(Effect::SetVoiceScopes(VoiceScopes::rooms(&[
//!     ("wolves", &wolves_alive), ("dead", &dead),
//! ])));
//! ```
//!
//! ## Enforcement is at the SFU, not in the UI
//!
//! A muted client that stops sending audio is a UI feature. A participant the SFU
//! will not forward is a *rule*. Phase 8's acceptance requires scope enforcement
//! verified **at the SFU** (doc 08 §5.C).
//!
//! ## Signaling rides the platform WebSocket
//!
//! As `PlatformCommand::Voice*` / `PlatformEvent::VoiceGrant`. Media does not.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/service.rs   VoiceService trait, JoinGrant, VoicePerms, VoiceStats
//! src/scopes.rs    VoiceScopes wire representation, participant permission model
//! src/livekit.rs   #[cfg(feature = "livekit")]  self-hosted adapter
//! src/managed.rs   #[cfg(feature = "managed")]  hosted-provider adapter
//! src/fake.rs      always available — local dev and tests never need an SFU
//! ```

#![forbid(unsafe_code)]
