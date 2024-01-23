use email_newsletter::{config, email_client::EmailClient, startup::run, telemetry::{get_subscriber, init_subscriber}};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_log::LogTracer;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    LogTracer::init().expect("Failed to set logger.");

    let subscriber = get_subscriber("email_newsletter".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = config::get_configuration();

    let conn_pool =
        PgPool::connect_lazy(&configuration.database.connection_string().expose_secret())
            .expect("Failed to connect to Postgres.");

    let address = format!(
        "{}:{}",
        configuration.app_settings.host, configuration.app_settings.port
    );

    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address.");

    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.authorization_token,
    );

    let listener = TcpListener::bind(address)?;

    run(listener, conn_pool, email_client)?.await?;
    Ok(())
}
