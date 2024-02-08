use crate::{
    auth::UserId,
    domain::SubscriberEmail,
    email_client::EmailClient,
    idempotency::{get_saved_response, save_res, try_processing, IdempotencyKey, NextAction},
    utils::{err_400, opaque_500_err, see_other},
};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String,
}

#[derive(Debug)]
struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(form, pool, email_client, user_id),
    fields(user_id=%*user_id)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    user_id: web::ReqData<UserId>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(err_400)?;
    let tx = match try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(opaque_500_err)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(resp) => {
            success_message().send();
            return Ok(resp);
        }
    };

    if let Some(saved_res) = get_saved_response(&pool, &idempotency_key, *user_id)
        .await
        .map_err(opaque_500_err)?
    {
        return Ok(saved_res);
    }
    let subscribers = get_confirmed_subscribers(&pool)
        .await
        .map_err(opaque_500_err)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(&subscriber.email, &title, &html_content, &text_content)
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })
                    .map_err(opaque_500_err)?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    error.message = %error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid",
                );
            }
        }
    }
    success_message().send();
    let resp = see_other("/admin/newsletters");
    let resp = save_res(&idempotency_key, *user_id, resp, tx)
        .await
        .map_err(opaque_500_err)?;

    Ok(resp)
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
                Ok(email) => Ok(ConfirmedSubscriber { email }),
                Err(error) => Err(anyhow::anyhow!(error)),
            })
            .collect();

    Ok(confirmed_subscribers)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}
