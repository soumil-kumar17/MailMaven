use crate::routes::{health_check, subscriptions::subscribe};
use actix_web::{dev::Server, middleware::Logger, web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;

pub fn run(listener: TcpListener, conn_pool: PgPool) -> Result<Server, std::io::Error> {
    let conn_pool = web::Data::new(conn_pool);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subsrciptions", web::post().to(subscribe))
            .app_data(conn_pool.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
