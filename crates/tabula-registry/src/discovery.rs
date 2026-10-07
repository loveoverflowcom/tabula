//! Read-only discovery/setup erasure for the DOM shell (doc 04 §3.2).
//!
//! The source of each fact and normalization result remains the unchanged
//! `ErasedGame` implementation. This vtable deliberately carries no canonical
//! match factory, preventing discovery from linking that authority by accident.

use crate::{
    ConfigForm, ConfigRejection, Locale, Messages, ModeSupport, NormalizedConfig, SetupRequest,
};
use tabula_core::BotLevel;
use tabula_game_api::{GameCapabilities, GameMetadata};

/// Catalog facts, typed configuration validation and separate-document handoff
/// declarations, without canonical match creation/restoration (I-5, ADR-011).
/// Import this trait explicitly from `discovery`; existing `ErasedGame` users and
/// their wildcard imports retain their original method-resolution surface.
pub trait ErasedDiscoveryGame: Send + Sync {
    fn metadata(&self) -> &'static GameMetadata;
    fn capabilities(&self) -> &'static GameCapabilities;
    /// Game-owned lightweight decorative art for Home/Library cards only.
    /// This is trusted compile-time SVG, never user input or a runtime asset pack.
    fn catalog_cover_svg(&self) -> Option<&'static str>;
    fn form(&self) -> &'static ConfigForm;
    fn modes(&self) -> &'static [ModeSupport];
    /// Explicit direct browser host declaration, never capability inference.
    fn direct_document(&self) -> bool;
    /// Package eligibility for the explicitly opted-in direct host consumer.
    fn direct_host_supported(&self) -> bool;
    /// Parse a candidate direct draft without asserting runtime availability.
    /// The server must re-normalize the request and validate its actual roster.
    fn normalize_direct(
        &self,
        seats: u8,
        draft: &crate::ConfigDraft,
    ) -> Result<NormalizedConfig, ConfigRejection>;
    /// This game's own visible copy for one locale.
    fn messages(&self, locale: Locale) -> Messages;
    /// The package's declared bot policy levels, independent of linked factories.
    /// This inventory cannot establish a gameplay host's bot-mode support.
    fn bot_levels(&self) -> Vec<BotLevel>;
    /// Parse, plan the seats, and run the game's own validation.
    ///
    /// # Errors
    /// [`ConfigRejection`] from parsing, the seat plan, or the module.
    fn normalize(&self, request: &SetupRequest) -> Result<NormalizedConfig, ConfigRejection>;
}

// Static qualified forwarding keeps the one existing adapter implementation
// as the behavior owner. In registered_discovery_games G is concrete Adapter<S>,
// so no dyn ErasedGame vtable or runtime factory is materialized here.
impl<G: crate::ErasedGame + ?Sized> ErasedDiscoveryGame for G {
    fn metadata(&self) -> &'static GameMetadata {
        <G as crate::ErasedGame>::metadata(self)
    }

    fn capabilities(&self) -> &'static GameCapabilities {
        <G as crate::ErasedGame>::capabilities(self)
    }

    fn catalog_cover_svg(&self) -> Option<&'static str> {
        <G as crate::ErasedGame>::catalog_cover_svg(self)
    }

    fn form(&self) -> &'static ConfigForm {
        <G as crate::ErasedGame>::form(self)
    }

    fn modes(&self) -> &'static [ModeSupport] {
        <G as crate::ErasedGame>::modes(self)
    }

    fn direct_document(&self) -> bool {
        <G as crate::ErasedGame>::direct_document(self)
    }

    fn direct_host_supported(&self) -> bool {
        <G as crate::ErasedGame>::direct_host_supported(self)
    }

    fn normalize_direct(
        &self,
        seats: u8,
        draft: &crate::ConfigDraft,
    ) -> Result<NormalizedConfig, ConfigRejection> {
        <G as crate::ErasedGame>::normalize_direct(self, seats, draft)
    }

    fn messages(&self, locale: Locale) -> Messages {
        <G as crate::ErasedGame>::messages(self, locale)
    }

    fn bot_levels(&self) -> Vec<BotLevel> {
        <G as crate::ErasedGame>::bot_levels(self)
    }

    fn normalize(&self, request: &SetupRequest) -> Result<NormalizedConfig, ConfigRejection> {
        <G as crate::ErasedGame>::normalize(self, request)
    }
}

/// The same catalog/query contract with a read-only discovery vtable.
pub type DiscoveryCatalog = crate::catalog::CatalogStorage<dyn ErasedDiscoveryGame>;
/// A discovery entry exposes facts/configuration and cannot create authority.
pub type DiscoveryCatalogEntry = crate::catalog::CatalogEntryStorage<dyn ErasedDiscoveryGame>;
