use crate::domain::SubscriberEmail;
use secrecy::{ExposeSecret, Secret};
use serde_yaml;
use serde_yaml::Error as YamlError;
use sqlx::postgres::PgConnectOptions;
use std::io::Error as IoError;

#[derive(serde::Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub app_settings: AppSettings,
    pub email_client: EmailClientSettings,
}

#[derive(serde::Deserialize)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    pub authorization_token: Secret<String>,
    pub timeout_ms: u64,
}

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    pub host: String,
    pub port: u16,
    pub name: String,
}

#[derive(serde::Deserialize)]
pub struct AppSettings {
    pub port: u16,
    pub host: String,
}

#[derive(Debug)]
pub enum ConfigError {
    FileOpenError(IoError),
    ParseError(YamlError),
}

impl From<IoError> for ConfigError {
    fn from(error: IoError) -> Self {
        ConfigError::FileOpenError(error)
    }
}

impl From<YamlError> for ConfigError {
    fn from(error: YamlError) -> Self {
        ConfigError::ParseError(error)
    }
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse_email(self.sender_email.clone())
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.timeout_ms)
    }
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<std::string::String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.name
        ))
    }

    pub fn get_db_options(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(self.host.as_ref())
            .port(self.port)
            .username(self.username.as_ref())
            .password(self.password.expose_secret())
            .database(self.name.as_ref())
    }
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    let file = std::fs::File::open("configuration.yaml")?;
    let scrape_cfg: Settings = serde_yaml::from_reader(file)?;
    Ok(scrape_cfg)
}
