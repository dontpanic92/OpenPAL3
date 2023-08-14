use std::path::PathBuf;

pub fn save_dir() -> PathBuf {
    #[cfg(any(windows, linux, macos))]
    {
        dirs::data_dir().unwrap().join("yaobow")
    }

    #[cfg(android)]
    {
        PathBuf::from(
            ndk_glue::native_activity()
                .external_data_path()
                .to_str()
                .unwrap(),
        )
        .join("yaobow")
    }

    #[cfg(vita)]
    {
        PathBuf::from("ux0:yaobow")
    }
}

pub fn config_dir() -> PathBuf {
    #[cfg(any(windows, linux, macos, android))]
    {
        dirs::config_dir().unwrap().join("yaobow")
    }

    #[cfg(vita)]
    {
        PathBuf::from("ux0:yaobow")
    }
}
