use serde_yaml;

#[derive(serde::Deserialize, Debug)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application_port: u16,
}

#[derive(serde::Deserialize, Debug)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub name: String,
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
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.name
        )
    }
}
