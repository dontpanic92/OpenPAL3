use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct YaobowConfig {
    pub asset_path: String,
}

impl YaobowConfig {
    pub fn load(config_name: &str, app_name: &str) -> anyhow::Result<YaobowConfig> {
        use crate::ydirs;

        let mut builder = config::Config::builder();

        if let Ok(expanded) = shellexpand::full(config_name) {
            let path = std::path::PathBuf::from(expanded.as_ref());
            if path.exists() {
                builder = builder.add_source(config::File::new(expanded.as_ref(), config::FileFormat::Toml));
            }
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
            Ok(config) => Ok(config.try_deserialize()?),
            Err(e) => anyhow::bail!("failed to load config: {}", e),
        }
    }
}
