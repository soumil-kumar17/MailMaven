use crate::auth::{validate_credentials, AuthError, Credentials};
use crate::routes::admin::dashboard::get_username;
use actix_web::{http::header::ContentType, web, HttpResponse};
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;
use std::fmt::Write;

use crate::{
    session_state::TypedSession,
    utils::{opaque_500_err, see_other},
};

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
    session: TypedSession,
    flash_msg: IncomingFlashMessages,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = session.get_user_id().map_err(opaque_500_err)?;
    if user_id.is_none() {
        return Ok(see_other("/login"));
    }
    let user_id = user_id.unwrap();
    if session.get_user_id().map_err(opaque_500_err)?.is_none() {
        return Ok(see_other("/login"));
    };
    if form.0.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error("You have entered two new passwords, the field values must match.")
            .send();
    }
    let mut html_msg = String::new();
    for m in flash_msg.iter() {
        writeln!(html_msg, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    let username = get_username(user_id, &pool).await.map_err(opaque_500_err)?;
    let credentials = Credentials {
        username,
        password: form.0.current_password,
    };
    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(see_other("/admin/password"))
            }
            AuthError::UnexpectedError(_) => Err(opaque_500_err(e).into()),
        };
    }
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8", content="text/html", http-equiv="content-type">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Admin Dashboard</title>
            </head>
            <body>
                {html_msg}
            </body>
        </html>"#
        )))
}
