//! Macroquad implementation of the renderer-neutral presentation contract. (doc 04 §6)
//!
//! Macroquad types are deliberately confined to this replaceable backend. The pure command-stream
//! interpreter in [`state`] establishes the effective transform, logical scissor, and inherited
//! primitive opacity before [`draw`] performs the framework calls.

#![forbid(unsafe_code)]

mod draw;
mod input;
mod renderer;
mod state;
mod support;
mod text;

pub use renderer::MacroquadRenderer;
