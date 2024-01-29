use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};
use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use anyhow::Context;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{types::chrono::Utc, PgPool, Postgres, Transaction};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse_name(value.name)?;
        let email = SubscriberEmail::parse_email(value.email)?;
        Ok(Self { name, email })
    }
}

pub struct TokenError(sqlx::Error);

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Database error occured while storing subscription token."
        )
    }
}

impl std::error::Error for TokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl std::fmt::Debug for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
        }
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, conn_pool, email_client),
    fields(
        subscriber_email = %form.email,
        subscriber_name= %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    conn_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;

    let mut transaction = conn_pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool.")?;

    let subscriber_id = insert_subscriber(&mut transaction, &conn_pool, &new_subscriber)
        .await
        .context("Failed to insert new subscriber in the database.")?;

    let subscription_token = generate_subscription_token();
    store_token(
        &mut transaction,
        &conn_pool,
        subscriber_id,
        &subscription_token,
    )
    .await
    .context("Failed to store subscription token in the database.")?;

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction.")?;

    send_confirmation_email(
        &email_client,
        &new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email.")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, conn_pool)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    conn_pool: &PgPool,
    new_subscriber: &NewSubscriber,
) -> Result<uuid::Uuid, sqlx::Error> {
    let subscriber_id = uuid::Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'confirmed')"#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(conn_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:#?}", e);
        e
    })?;
    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Sending confirmation email",
    skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: &NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let plain_text_content = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_content = format!(
        "Welcome to our newsletter!<br />\
        Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email(
            new_subscriber.email.clone(),
            "Welcome!",
            &html_content,
            &plain_text_content,
        )
        .await?;
    Ok(())
}

fn generate_subscription_token() -> String {
    let mut thread = thread_rng();
    std::iter::repeat_with(|| thread.sample(Alphanumeric))
        .map(char::from)
        .take(30)
        .collect()
}

#[tracing::instrument(name = "Saving subscription token in the database", skip(conn_pool))]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    conn_pool: &PgPool,
    subscriber_id: uuid::Uuid,
    subscription_token: &str,
) -> Result<(), TokenError> {
    sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    )
    .execute(conn_pool)
    .await
    .map_err(|e| TokenError(e))?;
    Ok(())
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}", e)?;
    let mut curr = e.source();
    while let Some(source) = curr {
        writeln!(f, "Caused by:\n\t{}", source)?;
        curr = source.source();
    }
    Ok(())
}
