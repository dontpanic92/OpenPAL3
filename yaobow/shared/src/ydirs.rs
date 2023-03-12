use std::path::PathBuf;

pub fn save_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from(
            ndk_glue::native_activity()
                .external_data_path()
                .to_str()
                .unwrap(),
        )
        .join("yaobow")
    }

    #[cfg(not(target_os = "android"))]
    {
        dirs::data_dir().unwrap().join("yaobow")
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap().join("yaobow")
}
