use crate::{
    auth::{basic_auth, validate_credentials, AuthError},
    domain::SubscriberEmail,
    email_client::EmailClient,
    routes::error_chain_fmt,
};
use actix_web::{
    http::{
        header::{self, HeaderValue},
        StatusCode,
    },
    web, HttpRequest, HttpResponse, ResponseError,
};
use anyhow::Context;
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
    let user_id = validate_credentials(credentials, &pool)
        .await
        .map_err(|err| match err {
            AuthError::InvalidCredentials(_) => PublishError::AuthError(err.into()),
            AuthError::UnexpectedError(_) => PublishError::UnexpectedError(err.into()),
        })?;
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
