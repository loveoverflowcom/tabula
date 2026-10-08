//! The only module in the workspace that names games.
//!
//! Every game-specific fact — form fields, units, summary wording, and the
//! modes this build can construct — lives in one adapter per game here. The
//! catalog and every platform consumer see only [`crate::erased::ErasedGame`],
//! which is what makes I-9 mechanically checkable.

#[cfg(feature = "game-chess")]
pub mod chess; // xtask-allow-game-id: the registry is the catalog's only game-naming boundary.
#[cfg(feature = "game-tiles")]
pub mod tiles; // xtask-allow-game-id: the registry is the catalog's only game-naming boundary.
pub mod werewolf; // xtask-allow-game-id: inert planned information, never runtime registration.
