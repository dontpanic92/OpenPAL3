//! Shared parsers for CEGUI-style UI data files used by PAL4 (and any
//! future title that ships its UI in the same format).
//!
//! All loaders take a parsed XML document (or a vfs path) and return
//! plain-data structs. They never touch the GPU; texture uploads are
//! the caller's responsibility (`yaobow_editor` does it via the imgui
//! texture cache; the runtime UI path does it via `radiance::Sprite`).

pub mod imageset;
pub mod layout;
pub mod scheme;
pub mod skin;
