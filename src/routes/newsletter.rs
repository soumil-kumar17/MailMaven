use crate::{domain::SubscriberEmail, email_client::EmailClient, routes::error_chain_fmt};
use actix_web::{
    http::{
        header::{self, HeaderMap, HeaderValue},
        StatusCode,
    },
    web, HttpRequest, HttpResponse, ResponseError,
};
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use base64::{engine::general_purpose, Engine};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                response.headers_mut().insert(
                    header::WWW_AUTHENTICATE,
                    HeaderValue::from_str(r#"Basic realm="publish""#).unwrap(),
                );
                response
            }
        }
    }
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(name = "Validating credentials", skip(credentials, pool))]
async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let (user_id, expected_password_hash) = 
        get_stored_credentials(&credentials.username, &pool)
            .await
            .map_err(PublishError::UnexpectedError)?
            .ok_or_else(|| PublishError::AuthError(anyhow::anyhow!("Unknown username.")))?;

    tokio::task::spawn_blocking(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")
    .map_err(PublishError::UnexpectedError)??;

    Ok(user_id)
}

#[tracing::instrument(
    name = "Verify password hash",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), PublishError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format.")
        .map_err(PublishError::UnexpectedError)?;
    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(PublishError::AuthError)
}

#[tracing::instrument(
    name = "Retrieving stored credentials from the database",
    skip(username, pool)
)]
pub async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"SELECT user_id, password_hash FROM users WHERE username = $1"#,
        username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to retrieve stored credentials")?
    .map(|row| (row.user_id, Secret::new(row.password_hash)));

    Ok(row)
}

fn basic_auth(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("Missing authorization header")?
        .to_str()
        .context("Authorization header was not valid UTF-8")?;
    let base64encoded = header_value
        .strip_prefix("Basic ")
        .context("Authorization header did not start with Basic")?;
    let decoded_bytes = general_purpose::STANDARD
        .decode(base64encoded)
        .context("Failed to base64 decode Authorization header")?;
    let decoded = String::from_utf8(decoded_bytes)
        .context("Authorization header did not contain valid UTF-8")?;
    let mut credentials = decoded.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("Authorization header did not contain a username"))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("Authorization header did not contain a password"))?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

#[tracing::instrument(
    name = "Publishing a newsletter issue",
    skip(body, pool, email_client, request),
    fields(
        username = tracing::field::Empty,
        user_id = tracing::field::Empty,
    )
)]
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_auth(request.headers()).map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", &tracing::field::display(&credentials.username));
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    let subscribers = get_confirmed_subscribers(&pool)
        .await
        .context("Failed to retrieve confirmed subscribers")?;
    for subscriber in subscribers {
        email_client
            .send_email(
                subscriber.email,
                &body.title,
                &body.content.html,
                &body.content.text,
            )
            .await
            .context("Failed to send email to subscriber")?;
    }

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(conn_pool)
)]
pub async fn get_confirmed_subscribers(
    conn_pool: &PgPool,
) -> Result<Vec<ConfirmedSubscriber>, sqlx::Error> {
    let confirmed_subscribers =
        sqlx::query!(r#"SELECT email FROM subscriptions WHERE status = 'confirmed'"#)
            .fetch_all(conn_pool)
            .await?;
    let confirmed_subscribers = confirmed_subscribers
        .into_iter()
        .filter_map(|row| match SubscriberEmail::parse_email(row.email) {
            Ok(email) => Some(ConfirmedSubscriber { email }),
            Err(error) => {
                tracing::warn!(
                    "Invalid email address used by confirmed subscriber:\n{}",
                    error
                );
                None
            }
        })
        .collect();

    Ok(confirmed_subscribers)
}
