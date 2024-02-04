use crate::{
    config::Settings,
    email_client::EmailClient,
    routes::{
        admin_dashboard, change_password, change_password_form, confirm, health_check, home, login,
        login_form, logout, newsletter::publish_newsletter, subscriptions::subscribe,
    },
};
use actix_session::{storage::RedisSessionStore, SessionMiddleware};
use actix_web::{
    cookie::Key,
    dev::Server,
    web::{self, Data},
    App, HttpServer,
};
use actix_web_flash_messages::{storage::CookieMessageStore, FlashMessagesFramework};
use secrecy::{ExposeSecret, Secret};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

#[derive(Debug)]
pub struct ApplicationBaseUrl(pub String);

#[derive(Clone, Debug)]
pub struct HmacSecretKey(pub Secret<String>);

impl Application {
    pub async fn build(config: Settings) -> Result<Self, anyhow::Error> {
        let conn_pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_lazy_with(config.database.get_db_options());

        let sender_email = config
            .email_client
            .sender()
            .expect("Invalid sender email address.");

        let timeout = config.email_client.timeout();
        let email_client = EmailClient::new(
            config.email_client.base_url,
            sender_email,
            config.email_client.authorization_token,
            timeout,
        );

        let address = format!("{}:{}", config.app_settings.host, config.app_settings.port);

        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            conn_pool,
            email_client,
            config.app_settings.base_url,
            config.app_settings.hmac_secret,
            config.redis_uri,
        )
        .await?;
        Ok(Self { port, server })
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

async fn run(
    listener: TcpListener,
    conn_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
    hmac_secret: Secret<String>,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    let conn_pool = web::Data::new(conn_pool);
    let email_client = web::Data::new(email_client);
    let key = Key::from(hmac_secret.expose_secret().as_bytes());
    let msg_store = CookieMessageStore::builder(key.clone()).build();
    let msg_framework = FlashMessagesFramework::builder(msg_store).build();
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;
    let server = HttpServer::new(move || {
        App::new()
            .wrap(msg_framework.clone())
            .wrap(SessionMiddleware::new(redis_store.clone(), key.clone()))
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subsrciptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletter", web::post().to(publish_newsletter))
            .route("/admin/dashboard", web::get().to(admin_dashboard))
            .route("/", web::get().to(home))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .route("/admin/password", web::get().to(change_password_form))
            .route("/admin/password", web::post().to(change_password))
            .route("/admin/logout", web::post().to(logout))
            .app_data(email_client.clone())
            .app_data(conn_pool.clone())
            .app_data(base_url.clone())
            .app_data(Data::new(HmacSecretKey(hmac_secret.clone())))
            .app_data(redis_uri.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
