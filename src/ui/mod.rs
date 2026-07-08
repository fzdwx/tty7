//! The GPUI view layer: the window shell (`app`), the split-pane tree (`pane`),
//! the command palette (`palette`), the settings panel (`settings`), and the
//! menu-bar / keymap / theme wiring (`keymap`, `theme`).
//!
//! Everything here may depend on `core` and `terminal`; nothing in those layers
//! depends back on `ui`.

pub mod app;
pub mod assets;
mod file_icons;
pub mod file_tree;
pub mod hints;
pub mod home;
pub mod keymap;
pub mod palette;
pub mod pane;
pub mod presets;
pub mod preview;
pub mod settings;
pub mod tab_strip;
pub mod theme;
pub mod workspace;
