use tabula_game_api::{A11yDescription, GameRules};

use crate::{FrameCtx, InputEvent, RenderList};

/// A phase-2 placeholder for a pack identity; backend-specific handles stay below this crate.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetPackRef {
    pub game: String,
    pub version: String,
}
impl AssetPackRef {
    #[must_use]
    pub fn new(game: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            game: game.into(),
            version: version.into(),
        }
    }
}

/// A command requested by presentation. It never mutates authoritative state itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intent<C> {
    pub command: C,
}

/// Client-only half of a game: projection plus local state becomes a [`RenderList`].
///
/// @ai.role game-presentation
/// @ai.domain presentation.game
/// @ai.pure true
/// @ai.invariant projection-only-input
/// @ai.invariant local-state-never-canonical
/// @ai.evidence render::tests::scoped_draws_respect_global_layer_order
#[allow(clippy::doc_markdown)]
pub trait GamePresentation: Send + 'static {
    type Rules: GameRules;
    type Local: Default;
    fn asset_pack() -> AssetPackRef;
    fn present(
        view: &<Self::Rules as GameRules>::View,
        local: &Self::Local,
        frame: &FrameCtx,
    ) -> RenderList;
    fn on_view_event(
        event: &<Self::Rules as GameRules>::ViewEvent,
        local: &mut Self::Local,
        frame: &FrameCtx,
    );
    fn on_input(
        input: &InputEvent,
        view: &<Self::Rules as GameRules>::View,
        local: &mut Self::Local,
    ) -> Option<Intent<<Self::Rules as GameRules>::Command>>;
    fn a11y(view: &<Self::Rules as GameRules>::View) -> A11yDescription;
}
