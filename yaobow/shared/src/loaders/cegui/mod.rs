//! Shared parsers for CEGUI-style UI data files used by PAL4 (and any
//! future title that ships its UI in the same format).
//!
//! Parsers (`imageset`, `layout`, `scheme`, `skin`) take a parsed XML
//! document (or a vfs path) and return plain-data structs — they never
//! touch the GPU. `ui_layout_handle` builds on top of the parsers to
//! upload imageset atlases into the imgui texture cache and expose
//! the parsed tree to script callers (PAL4 start menu, editor
//! previewer) through the `IUiLayoutHandle` COM interface.

pub mod imageset;
pub mod layout;
pub mod scheme;
pub mod skin;
pub mod ui_layout_handle;
