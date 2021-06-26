use anyhow::Result;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::Path;
use tracing::info;

#[derive(Clone, Debug, Deserialize)]
pub struct Redis {
    pub address: SocketAddr,
    pub pool_size: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Server {
    pub listen_address: SocketAddr,
    pub cors_origins: Option<Vec<String>>,
    pub cors_allow_any_origin: bool,
    pub path_prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Metrics {
    pub auth_enabled: bool,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
}

impl Metrics {
    pub fn make_basic_auth_header(self) -> String {
        let auth_string = [
            self.auth_username.unwrap_or_default(),
            ":".to_string(),
            self.auth_password.unwrap_or_default(),
        ]
        .concat();
        let encoded = base64::encode(auth_string);
        ["Basic ".to_string(), encoded].concat()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Channel {
    pub api_keys: Option<Vec<String>>,
    pub secret_key: String,
    pub ttl: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Settings {
    pub redis: Redis,
    pub server: Server,
    pub channel: Channel,
    pub metrics: Metrics,
}

impl Settings {
    pub fn new(config_file: Option<&Path>) -> Result<Self, ConfigError> {
        let mut s = Config::new();

        // Set defaults.
        s.set_default("redis.address", "127.0.0.1:6379")?;
        s.set_default("redis.pool_size", 1024)?;
        s.set_default("server.listen_address", "0.0.0.0:8080")?;
        s.set_default("server.cors_allow_any_origin", false)?;
        s.set_default("channel.ttl", 3600)?;
        s.set_default("channel.secret_key", "WAEgmUZx6H".to_string())?;
        s.set_default("metrics.auth_enabled", false)?;

        if let Some(config_file) = config_file {
            info!("Reading config file: {:?}", config_file);
            s.merge(File::from(config_file))?;
        }

        s.merge(Environment::with_prefix("WC").separator("__"))?;

        s.try_into()
    }
}
