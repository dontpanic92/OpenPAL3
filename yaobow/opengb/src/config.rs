use serde::Deserialize;
use shared::ydirs;

#[derive(Deserialize, Clone, Debug)]
pub struct OpenGbConfig {
    pub asset_path: String,
}

impl OpenGbConfig {
    #[cfg(not(target_os = "android"))]
    pub fn load(config_name: &str, app_name: &str) -> OpenGbConfig {
        let mut builder = config::Config::builder();

        if std::path::PathBuf::from(config_name).exists() {
            builder = builder.add_source(config::File::new(config_name, config::FileFormat::Toml));
        }

        let cfg = ydirs::config_dir().join(config_name);

        if cfg.exists() {
            builder = builder.add_source(config::File::new(
                cfg.to_string_lossy().to_string().as_str(),
                config::FileFormat::Toml,
            ));
        }

        builder = builder.add_source(config::Environment::with_prefix(app_name));

        match builder.build() {
            Ok(config) => config.try_deserialize().unwrap(),
            Err(e) => {
                panic!("Failed to load config");
            }
        }
    }
}
