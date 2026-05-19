//! Editor-only foreign services exposed to p7 scripts.

pub mod editor_host_context;
pub mod gizmo;
pub mod handles;
pub mod preview_registry;
pub mod previewer_hub;
pub mod resource_manager;
pub mod scene_handle;

pub use editor_host_context::EditorHostContext;
pub use handles::{AudioHandle, ImageHandle, ModelHandle, PreviewSession, VideoHandle};
pub use preview_registry::PreviewRegistry;
pub use previewer_hub::PreviewerHub;
pub use resource_manager::ResourceManager;
pub use scene_handle::{InspectorView, SceneHandle, ScenePreviewSession};
