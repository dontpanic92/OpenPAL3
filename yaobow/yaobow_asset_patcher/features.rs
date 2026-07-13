use cfg_aliases::cfg_aliases;

/// Same platform/backend aliasing every other crate's `build.rs` sets
/// up (see `yaobow/yaobow/features.rs`), needed because `radiance`'s
/// public `comdef` re-exports are gated behind these `cfg` aliases
/// rather than raw `target_os` checks.
pub fn enable_features() {
    cfg_aliases! {
        linux: { target_os = "linux" },
        macos: { target_os = "macos" },
        android: { target_os = "android" },
        vita: { target_os = "vita" },

        vulkan: { any(windows, linux, macos, android) },
        vitagl: { vita },
    }
}
