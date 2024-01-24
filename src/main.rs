use email_newsletter::{
    config,
    startup::Application,
    telemetry::{get_subscriber, init_subscriber},
};
use tracing_log::LogTracer;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    LogTracer::init().expect("Failed to set logger.");

    let subscriber = get_subscriber("email_newsletter".into(), "info".into(), std::io::stdout);

    init_subscriber(subscriber);

    let configuration = config::get_configuration().expect("Failed to read configuration.");

    let server = Application::build(configuration).await?;

    server.run_until_stopped().await?;

    Ok(())
}
