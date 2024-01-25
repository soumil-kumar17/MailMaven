use actix_web::{web, HttpResponse};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct QueryParameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(params))]
pub async fn confirm(
    params: web::Query<QueryParameters>,
    conn_pool: web::Data<PgPool>,
) -> HttpResponse {
    let subscriber_id = match get_subscriber_id(&conn_pool, &params.subscription_token).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    match subscriber_id {
        Some(id) => {
            if confirm_subscriber(&conn_pool, id).await.is_err() {
                return HttpResponse::InternalServerError().finish();
            }
            HttpResponse::Ok().finish()
        }
        None => HttpResponse::Unauthorized().finish(),
    }
}

#[tracing::instrument(
    name = "Get subscriber ID from token",
    skip(conn_pool, subscription_token)
)]
pub async fn get_subscriber_id(
    conn_pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<uuid::Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT subscriber_id FROM subscription_tokens
        WHERE subscription_token = $1
        "#,
        subscription_token
    )
    .fetch_optional(conn_pool)
    .await
    .map_err(|e| {
        tracing::error!("Error selecting subscriber ID: {:?}", e);
        e
    })?;
    Ok(result.map(|res| res.subscriber_id))
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(conn_pool, subscriber_id))]
pub async fn confirm_subscriber(
    conn_pool: &PgPool,
    subscriber_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions
        SET status = 'confirmed'
        WHERE id = $1
        "#,
        subscriber_id
    )
    .execute(conn_pool)
    .await
    .map_err(|e| {
        tracing::error!("Error updating status: {:?}", e);
        e
    })?;
    Ok(())
}
