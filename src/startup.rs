use crate::{
    email_client::EmailClient,
    routes::{health_check, subscriptions::subscribe},
};
use actix_web::{dev::Server, web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub fn run(
    listener: TcpListener,
    conn_pool: PgPool,
    email_client: EmailClient,
) -> Result<Server, std::io::Error> {
    let conn_pool = web::Data::new(conn_pool);
    let email_client = web::Data::new(email_client);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subsrciptions", web::post().to(subscribe))
            .app_data(email_client.clone())
            .app_data(conn_pool.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
