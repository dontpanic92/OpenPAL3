use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct OpenGbConfig {
    pub asset_path: String,
}

impl OpenGbConfig {
    pub fn load(config_name: &str, env_prefix: &str) -> OpenGbConfig {
        let mut builder = config::Config::builder();

        if std::path::PathBuf::from(config_name).exists() {
            builder = builder.add_source(config::File::new(config_name, config::FileFormat::Toml));
        }

        #[cfg(not(target_os = "android"))]
        {
            let cfg = dirs::config_dir()
                .unwrap()
                .join(config_name)
                .to_string_lossy()
                .to_string();

            println!("config: {}", cfg.as_str());
            if std::path::PathBuf::from(cfg.as_str()).exists() {
                builder =
                    builder.add_source(config::File::new(cfg.as_str(), config::FileFormat::Toml));
            }
        }

        builder = builder.add_source(config::Environment::with_prefix(env_prefix));

        match builder.build() {
            Ok(config) => config.try_deserialize().unwrap(),
            Err(e) => {
                panic!("Failed to load config");
            }
        }
    }
}
