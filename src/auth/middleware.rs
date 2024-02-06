use crate::{
    session_state::TypedSession,
    utils::{opaque_500_err, see_other},
};
use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    HttpMessage,
};
use actix_web::{error::InternalError, FromRequest};
use actix_web_lab::middleware::Next;
use std::ops::Deref;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct UserId(Uuid);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for UserId {
    type Target = Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub async fn reject_anonymous_users(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session = {
        let (http_request, payload) = req.parts_mut();
        TypedSession::from_request(http_request, payload).await
    }?;
    match session.get_user_id().map_err(opaque_500_err)? {
        Some(user_id) => {
            req.extensions_mut().insert(UserId(user_id));
            next.call(req).await
        }
        None => {
            let resp = see_other("/login");
            let err = anyhow::anyhow!("The user has not logged in");
            Err(InternalError::from_response(err, resp).into())
        }
    }
}
