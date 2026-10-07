//! Shared neutral public-occupant appearance for shell and canvas. (doc 04 §1.2)
//!
//! This semantic marker contains no account identity, role, seat, resource
//! loader or framework. A host's approved profile source owns actual images.

/// Shared neutral appearance while a public avatar is absent, loading, failed
/// or offline. Game state is a separate marker outside this image. (doc 04 §14)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarFallback {
    Human,
    Bot,
    Empty,
}

impl AvatarFallback {
    /// Small shared fallback glyph for DOM/canvas adapters. These are neutral
    /// public occupant markers and carry no seat, account or role information.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Human => "●",
            Self::Bot => "◇",
            Self::Empty => "○",
        }
    }

    /// Stable semantic name for adapters and accessible public state markers.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Bot => "bot",
            Self::Empty => "empty",
        }
    }
}
