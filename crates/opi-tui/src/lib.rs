//! Terminal User Interface library with differential rendering.
//!
//! Provides efficient text-based UI rendering that only updates changed regions,
//! along with text editor components and markdown rendering.

pub mod editor;
pub mod markdown;
pub mod render;

pub use editor::Editor;
pub use render::Renderer;
