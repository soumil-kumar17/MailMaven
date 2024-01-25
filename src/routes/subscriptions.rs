use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};
use actix_web::{web, HttpResponse};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{types::chrono::Utc, PgPool, Transaction};

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

#[derive(Debug)]
pub struct TokenError(sqlx::Error);

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Database error occured while storing subscription token.")
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
) -> Result<HttpResponse, actix_web::Error> {
    let new_subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return actix_web::Error,
    };

    let mut transaction = match conn_pool.begin().await {
        Ok(transaction) => transaction,
        Err(_) => return actix_web::Error,
    };

    match insert_subscriber(&mut transaction, &conn_pool, &new_subscriber).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    };

    let subscriber_id = match insert_subscriber(&mut transaction, &conn_pool, &new_subscriber).await
    {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return actix_web::Error,
    };

    let subscription_token = generate_subscription_token();
    store_token(
        &mut transaction,
        &conn_pool,
        subscriber_id,
        &subscription_token,
    )
    .await?;
    if transaction.commit().await.is_err() {
        return actix_web::Error;
    }

    let confirmation_link = "https://my-api.com/subscriptions/confirm";
    let plain_text_content = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_content = format!(
        "Welcome to our newsletter!<br />\
        Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );

    if send_confirmation_email(
        &email_client,
        &new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .is_err()
    {
        return actix_web::Error;
    }

    if email_client
        .send_email(
            new_subscriber.email,
            "Welcome!",
            &html_content,
            &plain_text_content,
        )
        .await
        .is_err()
    {
        return actix_web::Error;
    }

    HttpResponse::Ok()
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, conn_pool)
)]

pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
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
    .execute(transaction)
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
    transaction: &mut Transaction<'_, sqlx::Postgres>,
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
    .execute(transaction)
    .await
    .map_err(|e| {
        TokenError(e)
    })?;
    Ok(())
}
