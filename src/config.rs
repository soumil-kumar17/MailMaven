use secrecy::{ExposeSecret, Secret};
use serde_yaml;
use crate::domain::SubscriberEmail;

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
    //pub authorization_token: String,
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse_email(self.sender_email.clone())
    }
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

pub fn get_configuration() -> Settings {
    let file =
        std::fs::File::open("configuration.yaml").expect("Failed to open configuration file");
    let scrape_cfg: Settings =
        serde_yaml::from_reader(file).expect("Failed to read configuration file");
    //println!("{:?}", scrape_cfg);
    scrape_cfg
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
}
