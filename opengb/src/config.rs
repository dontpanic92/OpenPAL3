use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct OpenGbConfig {
    pub asset_path: String,
}

impl OpenGbConfig {
    pub fn load(config_name: &str, env_prefix: &str) -> OpenGbConfig {
        let mut settings = config::Config::default();
        settings
            .merge(config::File::with_name(config_name))
            .unwrap()
            .merge(config::Environment::with_prefix(env_prefix))
            .unwrap();
        settings.try_into::<OpenGbConfig>().unwrap().clone()
    }
}
