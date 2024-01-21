use email_newsletter::telemetry::{get_subscriber, init_subscriber};
use email_newsletter::{config, startup};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_log::LogTracer;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    LogTracer::init().expect("Failed to set logger.");

    let subscriber = get_subscriber("email_newsletter".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = config::get_configuration();

    let conn_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres.");

    let address = format!("127.0.0.1:{}", configuration.application_port);

    let listener = TcpListener::bind(address)?;

    startup::run(listener, conn_pool)?.await?;
    Ok(())
}
