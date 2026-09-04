use glam::Vec2;

/// A backend-neutral pointer coordinate with finite axes.
///
/// Viewport containment is intentionally not part of this type: platform
/// interactions may report a pointer outside the logical viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerPosition(Vec2);

/// Why a pointer coordinate cannot cross the presentation boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerPositionError;

impl core::fmt::Display for PointerPositionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("pointer coordinates must be finite")
    }
}

impl std::error::Error for PointerPositionError {}

impl PointerPosition {
    /// Validates both logical pointer axes without normalizing or clamping.
    ///
    /// @ai.role trust-boundary
    /// @ai.domain presentation.input
    /// @ai.invariant finite-pointer-coordinates
    /// @ai.evidence crate::input::tests::pointer_position_rejects_non_finite_axes
    #[allow(clippy::doc_markdown)]
    pub fn new(value: Vec2) -> Result<Self, PointerPositionError> {
        value
            .is_finite()
            .then_some(Self(value))
            .ok_or(PointerPositionError)
    }

    #[must_use]
    pub const fn get(self) -> Vec2 {
        self.0
    }
}

impl TryFrom<Vec2> for PointerPosition {
    type Error = PointerPositionError;

    fn try_from(value: Vec2) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Backend-normalized input; no framework enums cross this boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Pointer {
        position: PointerPosition,
        button: PointerButton,
        phase: PointerPhase,
    },
    Key {
        key: Key,
        pressed: bool,
    },
    Focus(bool),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Enter,
    Space,
    Escape,
    Tab,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_position_accepts_finite_coordinates_without_clamping() {
        let position = PointerPosition::try_from(Vec2::new(-1.0e30, 1.0e30)).unwrap();
        assert_eq!(position.get(), Vec2::new(-1.0e30, 1.0e30));
    }

    #[test]
    fn pointer_position_rejects_non_finite_axes() {
        for position in [
            Vec2::new(f32::NAN, 0.0),
            Vec2::new(0.0, f32::NAN),
            Vec2::new(f32::INFINITY, 0.0),
            Vec2::new(0.0, f32::INFINITY),
            Vec2::new(f32::NEG_INFINITY, 0.0),
            Vec2::new(0.0, f32::NEG_INFINITY),
        ] {
            assert_eq!(
                PointerPosition::try_from(position),
                Err(PointerPositionError)
            );
        }
    }
}
