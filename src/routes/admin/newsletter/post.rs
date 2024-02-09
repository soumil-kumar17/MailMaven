use crate::{
    auth::UserId,
    domain::SubscriberEmail,
    idempotency::{save_res, try_processing, IdempotencyKey, NextAction},
    utils::{err_400, opaque_500_err, see_other},
};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::{Context, Ok};
use sqlx::{PgPool, Transaction};
use uuid::Uuid;

use actix_web::error::Result as actix_web_Result;
use anyhow::Result as anyhow_Result;
use core::result::Result::Ok as core_Ok;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String,
}

#[derive(Debug)]
struct ConfirmedSubscriber {
    #[allow(dead_code)]
    email: SubscriberEmail,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip_all,
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    user_id: web::ReqData<UserId>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(err_400)?;
    let mut tx = match try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(opaque_500_err)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(resp) => {
            success_message().send();
            return anyhow_Result::Ok(resp);
        }
    };

    let issue_id = insert_newsletter_issue(&mut tx, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(opaque_500_err)?;
    enqueue_delivery_tasks(&mut tx, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(opaque_500_err)?;

    let resp = see_other("/admin/newsletters");
    let resp = save_res(&idempotency_key, *user_id, resp, tx)
        .await
        .map_err(opaque_500_err)?;

    success_message().send();
    actix_web_Result::Ok(resp)
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(conn_pool)
)]
pub async fn get_confirmed_subscribers(
    conn_pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers: Vec<Result<ConfirmedSubscriber, anyhow::Error>> =
        sqlx::query!(r#"SELECT email FROM subscriptions WHERE status = 'confirmed'"#)
            .fetch_all(conn_pool)
            .await?
            .into_iter()
            .map(|row| match SubscriberEmail::parse_email(row.email) {
                core_Ok(email) => Ok(ConfirmedSubscriber { email }),
                Err(error) => Err(anyhow::anyhow!(error)),
            })
            .collect();

    Ok(confirmed_subscribers)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been accepted and will shortly be published!")
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    title: &str,
    html_content: &str,
    text_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (newsletter_issue_id, title, html_content, text_content, published_at)
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        html_content,
        text_content
    )
    .execute(&mut **transaction)
    .await?;
    sqlx::error::Result::Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (newsletter_issue_id, subscriber_email)
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id,
    )
    .execute(&mut **transaction)
    .await?;
    anyhow_Result::Ok(())
}
