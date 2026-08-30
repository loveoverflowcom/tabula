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
    command: C,
}

impl<C> Intent<C> {
    #[must_use]
    pub const fn new(command: C) -> Self {
        Self { command }
    }

    #[must_use]
    pub const fn command(&self) -> &C {
        &self.command
    }

    #[must_use]
    pub fn into_command(self) -> C {
        self.command
    }
}

/// Client-only half of a game: projection plus local state becomes a [`RenderList`].
///
/// Its public transition methods deliberately name [`GameRules::View`] and [`GameRules::ViewEvent`]
/// only. Canonical [`GameRules::State`] is not an input to this boundary, while `Local` is owned by
/// the client and never returned as a rule outcome.
///
/// ```compile_fail
/// use tabula_game_api::GameRules;
/// use tabula_presentation::{FrameCtx, GamePresentation, RenderList};
///
/// fn state_cannot_be_presented<P: GamePresentation>(
///     state: &<P::Rules as GameRules>::State,
///     local: &P::Local,
///     frame: &FrameCtx,
/// ) -> RenderList {
///     P::present(state, local, frame)
/// }
/// ```
///
/// @ai.role game-presentation
/// @ai.domain presentation.game
/// @ai.related tabula_game_api::GameRules
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
