use appstate::DbState;
use auth::token;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use common::webhook::Webhook;
use error::api_error::{ApiError, ApiResult};

use crate::types;

pub async fn register_webhook(
    token: token::Token,
    State(db): DbState,
    Json(input): Json<types::RegisterWebhookRequest>,
) -> ApiResult<Json<types::RegisterWebhookResponse>> {
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", "", StatusCode::UNAUTHORIZED));
    }

    let id = db
        .register_webhook(Webhook {
            id: None,
            action: input.action,
            callback_url: input.callback_url,
            name: input.name,
        })
        .await?;

    Ok(Json(types::RegisterWebhookResponse { id }))
}

pub async fn get_webhook(
    token: token::Token,
    Path(id): Path<String>,
    State(db): DbState,
) -> ApiResult<Json<types::GetWebhookResponse>> {
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", "", StatusCode::UNAUTHORIZED));
    }

    let w = db.get_webhook(&id).await?;
    Ok(Json(types::GetWebhookResponse {
        id: w.id.unwrap_or_default(),
        action: w.action,
        callback_url: w.callback_url,
        name: w.name,
    }))
}

pub async fn get_all_webhooks(
    token: token::Token,
    State(db): DbState,
) -> ApiResult<Json<types::GetAllWebhooksResponse>> {
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", "", StatusCode::UNAUTHORIZED));
    }

    let w = db.get_all_webhooks().await?;
    Ok(Json(types::GetAllWebhooksResponse(w)))
}

pub async fn delete_webhook(
    token: token::Token,
    Path(id): Path<String>,
    State(db): DbState,
) -> ApiResult<()> {
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", "", StatusCode::UNAUTHORIZED));
    }

    db.delete_webhook(&id).await?;
    Ok(())
}

pub async fn test_webhook(
    token: token::Token,
    Path(id): Path<String>,
    State(db): DbState,
) -> ApiResult<()> {
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", "", StatusCode::UNAUTHORIZED));
    }

    let w = db.get_webhook(&id).await?;
    let client = reqwest::Client::new();
    let resp = client
        .post(&w.callback_url)
        .json("Test Payload")
        .send()
        .await
        .map_err(|e| ApiError::new(&e.to_string(), "", StatusCode::from_u16(500).unwrap()))?;

    match resp.status() {
        a if a.as_u16() < 300 => Ok(()),
        a => Err(ApiError::new(&resp.text().await.unwrap_or_default(), "", a)),
    }
}
