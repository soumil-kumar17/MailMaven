#![allow(dead_code)]
use crate::startup::HmacSecretKey;
use actix_web::{http::header::ContentType, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use sha2::Sha256;
use std::fmt::Write;

#[derive(serde::Deserialize)]
pub struct QueryParams {
    error: String,
    hmac_tag: String,
}

impl QueryParams {
    fn verify(self, secret: &HmacSecretKey) -> Result<String, anyhow::Error> {
        let expected_tag = hex::decode(self.hmac_tag)?;
        let query_string = format!("error={}", urlencoding::Encoded::new(&self.error));
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.0.expose_secret().as_bytes()).unwrap();
        mac.update(query_string.as_bytes());
        mac.verify_slice(&expected_tag)?;

        Ok(self.error)
    }
}

pub async fn login_form(flash_msg: IncomingFlashMessages) -> HttpResponse {
    let mut html_err = String::new();
    for m in flash_msg.iter() {
        writeln!(html_err, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8", content="text/html", http-equiv="content-type">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <title>Login</title>
            </head>
            <body>
                {html_err}
                <form action="/login" method="post">
                    <label>Username
                        <input type="text" name="username" placeholder="Enter username">
                    </label>
                    <label>Password
                        <input type="password" name="password" placeholder="Enter password">
                    </label>
                    <button type="submit">Login</button>
                </form>
            </body>
            </html>"#
        ))
}
