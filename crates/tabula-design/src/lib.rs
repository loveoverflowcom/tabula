//! # `tabula-design` — semantic design tokens
//!
//! > ## PHASE 2
//!
//! One semantic language across DOM and canvas is the only way the product feels
//! like one product (ADR-018). Tokens are defined **once, in Rust**, and adapted
//! to CSS custom properties (Leptos) and a resolved `Theme` struct (Macroquad).
//!
//! ## Generation, not duplication (doc 04 §8.1)
//!
//! ```text
//! tokens.toml  ──xtask gen-tokens──┬─→ crates/tabula-design/src/generated.rs  (const Theme)
//!  (source of                      ├─→ apps/web/style/tokens.css             (:root --sys-*)
//!   truth, root)                   └─→ docs/ui/tokens.json                   (design tools)
//! ```
//!
//! All three outputs are **committed**, and CI fails if they are stale.
//! Four themes are generated: `light`, `dark`, `hc-light`, `hc-dark`.
//!
//! **No hex literals anywhere outside this crate.** `xtask check-no-raw-colors`
//! greps for hex literals and `Color::new(` in `apps/` and `games/`.
//!
//! ## Three tiers (doc 04 §7.2)
//!
//! ```text
//! Tier 1 — reference     raw values; NOBODY uses these directly
//!                        ref.palette.warm.40, ref.type.display.size.3
//! Tier 2 — system        what code and design use
//!                        sys.color.surface, sys.color.turn-active, sys.shape.card
//! Tier 3 — component     only where a component must deviate, with a written reason
//!                        comp.button.container-color
//! ```
//!
//! ## The token set (doc 04 §7.3)
//!
//! ```rust,ignore
//! pub struct Theme {
//!     pub color: ColorTokens, pub type_: TypeTokens, pub shape: ShapeTokens,
//!     pub space: SpaceTokens, pub elevation: ElevationTokens, pub motion: MotionTokens,
//!     pub state: StateLayerTokens, pub density: Density, pub focus: FocusTokens,
//! }
//! ```
//!
//! `ColorTokens` has the usual surface/brand/feedback roles **plus the ones that
//! make a board legible** — these are the tokens that justify building a design
//! system for a game platform rather than reusing a web one:
//!
//! ```rust,ignore
//! pub turn_active: Color,    // whose turn it is
//! pub turn_waiting: Color,
//! pub legal_target: Color,   // a legal destination
//! pub illegal_target: Color,
//! pub selected: Color,
//! pub last_action: Color,    // "the opponent just did this"
//! pub threat: Color,         // check, danger, being voted
//! pub hidden: Color,         // card backs, fog, unknown role
//! pub team: [Color; 8],      // colorblind-safe set
//! pub seat_marker: [Color; 8],
//! ```
//!
//! Scales: space `0,2,4,8,12,16,20,24,32,40,48,64`; shape
//! `none/xs/sm/md/lg/xl/full` plus semantic `card/board/token/sheet/button/chip`.
//! Type roles: `display|headline|title|body|label` × `lg/md/sm`, plus `mono.md/sm`
//! with **tabular figures required** — clocks that reflow while ticking are
//! unreadable.
//!
//! ## Motion is semantic, not numeric (doc 04 §9.2)
//!
//! Presenters ask for `motion.piece-move`, never for `280ms ease-out`. The token
//! carries a spring, a duration, an easing, and a stagger:
//!
//! ```text
//! motion.piece-move    spring_weighty, slight arc, 0.94→1.0 scale on land
//! motion.card-deal     spring_standard, dur_medium, 40 ms stagger per card
//! motion.reveal        dur_long, two-phase lift + flip with a highlight sweep
//! motion.phase-change  dur_long tonal wash + title card — MUST be skippable
//! motion.invalid       120 ms 3-cycle shake + danger flash — NEVER a modal
//! motion.win / .lose   dur_xlong choreographed, always skippable by tap
//! ```
//!
//! `ReducedMotion` is a first-class token group, not an afterthought:
//! `duration_scale`, `prefer_fade`, `disable_ambient`, and `keep_informative`
//! (default **true** — a piece move carries information, so it is shortened
//! rather than removed).
//!
//! ## Per-game accent (doc 04 §8.4)
//!
//! ```toml
//! # games/chess/game.toml
//! [theme]
//! accent      = "#3E7B5A"   # tonal palette DERIVED AT BUILD TIME
//! board_light = "sys.surface.container-lowest"
//! mood        = "calm"      # calm | lively | tense — selects a motion profile
//! ```
//!
//! Precomputing the tonal palette at build time is why this crate stays
//! dependency-free: no runtime HCT colour maths. A game may supply source
//! colours; it may **not** override semantic roles.
//!
//! ## Module layout when this becomes real
//!
//! ```text
//! src/tokens.rs     Theme + every *Tokens struct  (hand-written, the schema)
//! src/generated.rs  const Theme values, 4 schemes (GENERATED — do not edit)
//! src/css.rs        #[cfg(feature = "css")]     CSS custom-property emitter
//! src/runtime.rs    #[cfg(feature = "runtime")] resolved theme for the canvas
//! src/color.rs      Color type + the small amount of maths we allow
//! ```

#![forbid(unsafe_code)]
