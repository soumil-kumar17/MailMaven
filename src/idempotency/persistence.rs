use super::IdempotencyKey;
use actix_web::{body::to_bytes, http::StatusCode, HttpResponse};
use sqlx::{postgres::PgHasArrayType, PgPool};
use uuid::Uuid;

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

impl PgHasArrayType for HeaderPairRecord {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_header_pair")
    }
}

pub async fn get_saved_response(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let saved_response = sqlx::query!(
        r#"
        SELECT
            response_status_code as "response_status_code!",
            response_headers as "response_headers!: Vec<HeaderPairRecord>",
            response_body as "response_body!"
        FROM idempotency
        WHERE
            user_id = $1 AND
            idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref()
    )
    .fetch_optional(pool)
    .await?;
    if let Some(r) = saved_response {
        let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;
        let mut response = HttpResponse::build(status_code);
        for HeaderPairRecord { name, value } in r.response_headers {
            response.append_header((name, value));
        }
        Ok(Some(response.body(r.response_body)))
    } else {
        Ok(None)
    }
}

pub async fn save_res(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
    http_res: HttpResponse,
) -> Result<HttpResponse, anyhow::Error> {
    let (resp_head, body) = http_res.into_parts();
    let body = to_bytes(body).await.map_err(|e| anyhow::anyhow!("{}", e))?;
    let status_code = resp_head.status().as_u16() as i16;
    let headers = {
        let mut headers = Vec::with_capacity(resp_head.headers().len());
        for (name, value) in resp_head.headers().iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();
            headers.push(HeaderPairRecord { name, value });
        }
        headers
    };
    sqlx::query_unchecked!(
        r#"
        INSERT INTO idempotency (
            user_id,
            idempotency_key,
            response_status_code,
            response_headers,
            response_body,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, now())
        "#,
        user_id,
        idempotency_key.as_ref(),
        status_code,
        headers,
        body.as_ref()
    )
    .execute(pool)
    .await?;

    let http_resp = resp_head.set_body(body).map_into_boxed_body();
    Ok(http_resp)
}
