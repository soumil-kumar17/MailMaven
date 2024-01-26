use crate::routes::error_chain_fmt;
use crate::{domain::SubscriberEmail, email_client::EmailClient};
use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
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
    #[error("{1}")]
    UnexpectedError(#[source] Box<dyn std::error::Error>, String),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::UnexpectedError(_, _) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, PublishError> {
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(|err| {
        PublishError::UnexpectedError(
            Box::new(err),
            "Failed to get list of confirmed subscribers".into(),
        )
    })?;
    for subscriber in subscribers {
        email_client
            .send_email(
                subscriber.email,
                &body.title,
                &body.content.html,
                &body.content.text,
            )
            .await
            .map_err(|err| {
                PublishError::UnexpectedError(
                    Box::new(err),
                    "Unable to retrieve email details from confirmed subscriber details".into(),
                )
            })?;
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
    struct Row {
        email: String,
    }

    let confirmed_subscribers = sqlx::query_as!(
        Row,
        r#"SELECT email FROM subscriptions WHERE status = 'confirmed'"#
    )
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
