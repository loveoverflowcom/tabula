use glam::Vec2;

/// Backend-normalized input; no framework enums cross this boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Pointer {
        position: Vec2,
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
