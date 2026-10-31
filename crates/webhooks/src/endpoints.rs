use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use kellnr_appstate::{DbState, SettingsState};
use kellnr_auth::token;
use kellnr_common::webhook::Webhook;
use kellnr_error::api_error::{ApiError, ApiResult};
use tracing::{trace, warn};
use uuid::Uuid;

use crate::client::{Delivery, DeliveryError, WebhookClient, truncated_body};
use crate::signature::decode_secret;
use crate::target::{is_blocked_url, validate_callback_url};
use crate::types;

/// Register a new webhook (admin only)
#[utoipa::path(
    post,
    path = "/",
    tag = "webhooks",
    request_body = types::RegisterWebhookRequest,
    responses(
        (status = 200, description = "Webhook registered successfully", body = types::RegisterWebhookResponse),
        (status = 400, description = "Invalid callback URL, target address not permitted or invalid secret"),
        (status = 401, description = "Admin access required")
    ),
    security(("cargo_token" = []))
)]
pub async fn register_webhook(
    token: token::Token,
    State(db): DbState,
    State(settings): SettingsState,
    Json(input): Json<types::RegisterWebhookRequest>,
) -> ApiResult<Json<types::RegisterWebhookResponse>> {
    trace!(user = %token.user, event = ?input.event, "Registering webhook");
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", StatusCode::UNAUTHORIZED));
    }

    let url = validate_callback_url(&input.callback_url)
        .map_err(|e| ApiError::new(&e, StatusCode::BAD_REQUEST))?;
    if is_blocked_url(&url, settings.webhooks.block_private_targets) {
        return Err(ApiError::new(
            "Callback URL target address is not permitted",
            StatusCode::BAD_REQUEST,
        ));
    }
    if let Some(secret) = &input.secret {
        decode_secret(secret).map_err(|e| ApiError::new(&e, StatusCode::BAD_REQUEST))?;
    }

    let id = db
        .register_webhook(Webhook {
            id: None,
            event: input.event,
            callback_url: input.callback_url,
            name: input.name,
            secret: input.secret,
        })
        .await?;

    Ok(Json(types::RegisterWebhookResponse { id }))
}

/// Get a webhook by ID (admin only)
#[utoipa::path(
    get,
    path = "/{id}",
    tag = "webhooks",
    params(
        ("id" = String, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "Webhook details", body = types::GetWebhookResponse),
        (status = 401, description = "Admin access required")
    ),
    security(("cargo_token" = []))
)]
pub async fn get_webhook(
    token: token::Token,
    Path(id): Path<String>,
    State(db): DbState,
) -> ApiResult<Json<types::GetWebhookResponse>> {
    trace!(user = %token.user, webhook_id = %id, "Getting webhook");
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", StatusCode::UNAUTHORIZED));
    }

    Ok(Json(db.get_webhook(&id).await?.into()))
}

/// List all webhooks (admin only)
#[utoipa::path(
    get,
    path = "/",
    tag = "webhooks",
    responses(
        (status = 200, description = "List of all webhooks", body = types::GetAllWebhooksResponse),
        (status = 401, description = "Admin access required")
    ),
    security(("cargo_token" = []))
)]
pub async fn get_all_webhooks(
    token: token::Token,
    State(db): DbState,
) -> ApiResult<Json<types::GetAllWebhooksResponse>> {
    trace!(user = %token.user, "Listing all webhooks");
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", StatusCode::UNAUTHORIZED));
    }

    let w = db.get_all_webhooks().await?;
    Ok(Json(types::GetAllWebhooksResponse(
        w.into_iter().map(Into::into).collect(),
    )))
}

/// Delete a webhook (admin only)
#[utoipa::path(
    delete,
    path = "/{id}",
    tag = "webhooks",
    params(
        ("id" = String, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "Webhook deleted successfully"),
        (status = 401, description = "Admin access required")
    ),
    security(("cargo_token" = []))
)]
pub async fn delete_webhook(
    token: token::Token,
    Path(id): Path<String>,
    State(db): DbState,
) -> ApiResult<()> {
    trace!(user = %token.user, webhook_id = %id, "Deleting webhook");
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", StatusCode::UNAUTHORIZED));
    }

    db.delete_webhook(&id).await?;
    Ok(())
}

/// Test a webhook by sending a test payload (admin only)
#[utoipa::path(
    post,
    path = "/{id}/test",
    tag = "webhooks",
    params(
        ("id" = String, Path, description = "Webhook ID")
    ),
    responses(
        (status = 200, description = "Test payload sent successfully"),
        (status = 400, description = "Invalid callback URL or target address not permitted"),
        (status = 401, description = "Admin access required"),
        (status = 500, description = "Webhook callback request failed"),
        (status = 502, description = "Webhook callback returned a status other than 2xx, which is contained in the error message")
    ),
    security(("cargo_token" = []))
)]
pub async fn test_webhook(
    token: token::Token,
    Path(id): Path<String>,
    State(db): DbState,
    State(settings): SettingsState,
) -> ApiResult<()> {
    trace!(user = %token.user, webhook_id = %id, "Testing webhook");
    if !token.is_admin {
        return Err(ApiError::new("Unauthorized", StatusCode::UNAUTHORIZED));
    }

    let w = db.get_webhook(&id).await?;
    let client = WebhookClient::new(&settings.webhooks)
        .map_err(|e| ApiError::from_err(&e, StatusCode::INTERNAL_SERVER_ERROR))?;
    let msg_id = format!("msg_test_{}", Uuid::new_v4().simple());
    let delivery = Delivery {
        id: &msg_id,
        payload: "Test Payload",
        secret: w.secret.as_deref(),
    };
    let resp = client
        .post(&w.callback_url, &delivery)
        .await
        .map_err(|e| delivery_error_to_api_error(&id, &e))?;

    let status = resp.status();
    if status.as_u16() < 300 {
        return Ok(());
    }

    warn!(
        webhook_id = %id,
        "Webhook test callback returned status {status}. Body: {:?}",
        truncated_body(resp).await
    );
    // Only the status code is passed on in the message, the response body of
    // the callback host is never returned to the caller. The callback status
    // is not used as the response status, as e.g. a 401 of the callback would
    // look like a failed login to Kellnr.
    Err(ApiError::new(
        &format!("Webhook callback returned status {}", status.as_u16()),
        StatusCode::BAD_GATEWAY,
    ))
}

fn delivery_error_to_api_error(webhook_id: &str, err: &DeliveryError) -> ApiError {
    match err {
        DeliveryError::InvalidUrl(_)
        | DeliveryError::BlockedTarget
        | DeliveryError::InvalidSecret(_) => {
            ApiError::new(&err.to_string(), StatusCode::BAD_REQUEST)
        }
        DeliveryError::Request(_) | DeliveryError::Payload(_) => {
            warn!(webhook_id, "Webhook test callback failed: {err}");
            ApiError::new(
                "Webhook callback request failed",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

#[cfg(test)]
mod endpoint_tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use axum::response::Response;
    use axum::routing::{delete, get, post};
    use hyper::header;
    use kellnr_appstate::AppStateData;
    use kellnr_common::account_name::UserName;
    use kellnr_common::plain_token::PlainToken;
    use kellnr_common::webhook::WebhookEvent;
    use kellnr_db::{ConString, Database, DbProvider, SqliteConString, UserFlags};
    use serde::de::DeserializeOwned;
    use tower::ServiceExt;

    use super::*;
    use crate::tests::{get_test_listener, get_test_listener_with_response};
    use crate::types::{GetAllWebhooksResponse, GetWebhookResponse, RegisterWebhookResponse};

    const ADMIN_TOKEN: &str = "jkjkashd09128u3019283o1i3j";
    const NON_ADMIN_TOKEN: &str = "kjas09ed8o1i23k1jh";

    #[tokio::test]
    async fn test_register_webhook() {
        let (router, db) = get_app().await;

        let payload = "{\"type\": \"crate_add\", \"callback_url\": \"http://my-service:8000\"}";

        let response = router
            .clone()
            .oneshot(
                Request::post("/api/v1/webhook")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(200, response.status().as_u16());

        let response: RegisterWebhookResponse = parse_response(response).await;

        let webhook = db.get_webhook(&response.id).await.unwrap();
        assert_eq!(webhook.event, WebhookEvent::CrateAdd);
        assert_eq!(webhook.callback_url, "http://my-service:8000".to_string());
    }

    #[tokio::test]
    async fn test_register_webhook_invalid_callback_url() {
        let (router, db) = get_app().await;

        for url in [
            "not a url",
            "file:///etc/passwd",
            "ftp://my-service",
            "http://169.254.169.254/latest/meta-data",
        ] {
            let payload = format!("{{\"type\": \"crate_add\", \"callback_url\": \"{url}\"}}");

            let response = router
                .clone()
                .oneshot(
                    Request::post("/api/v1/webhook")
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::AUTHORIZATION, ADMIN_TOKEN)
                        .body(Body::from(payload))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(400, response.status().as_u16(), "{url}");
        }

        assert!(db.get_all_webhooks().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_register_webhook_with_secret_is_never_returned() {
        let (router, db) = get_app().await;
        let secret = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";

        let payload = format!(
            "{{\"type\": \"crate_add\", \"callback_url\": \"http://my-service:8000\", \"secret\": \"{secret}\"}}"
        );
        let response = router
            .clone()
            .oneshot(
                Request::post("/api/v1/webhook")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(200, response.status().as_u16());
        let response: RegisterWebhookResponse = parse_response(response).await;
        let id = response.id;
        assert_eq!(
            Some(secret.to_string()),
            db.get_webhook(&id).await.unwrap().secret
        );

        for uri in [
            format!("/api/v1/webhook/{id}"),
            "/api/v1/webhook".to_string(),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::get(uri)
                        .header(header::AUTHORIZATION, ADMIN_TOKEN)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(200, response.status().as_u16());
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body = String::from_utf8_lossy(&body);
            assert!(!body.contains("MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw"));
            assert!(!body.contains("\"secret\""));
            assert!(body.contains("\"has_secret\":true"));
        }
    }

    #[tokio::test]
    async fn test_register_webhook_invalid_secret() {
        let (router, db) = get_app().await;

        let payload = "{\"type\": \"crate_add\", \"callback_url\": \"http://my-service:8000\", \"secret\": \"too-short\"}";
        let response = router
            .clone()
            .oneshot(
                Request::post("/api/v1/webhook")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(400, response.status().as_u16());
        assert!(db.get_all_webhooks().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_register_webhook_non_admin() {
        let (router, _) = get_app().await;

        let payload = "{\"type\": \"crate_add\", \"callback_url\": \"http://my-service:8000\"}";

        let response = router
            .clone()
            .oneshot(
                Request::post("/api/v1/webhook")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(401, response.status().as_u16());
    }

    #[tokio::test]
    async fn test_get_webhook() {
        let (router, db) = get_app().await;

        let id = db.register_webhook(sample_webhook()).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/webhook/{id}"))
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(200, response.status().as_u16());

        let response: GetWebhookResponse = parse_response(response).await;

        assert_eq!(response.event, WebhookEvent::CrateUpdate);
        assert_eq!(response.callback_url, sample_webhook().callback_url);
        assert_eq!(response.name, sample_webhook().name);
    }

    #[tokio::test]
    async fn test_get_webhook_non_admin() {
        let (router, db) = get_app().await;

        let id = db.register_webhook(sample_webhook()).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/webhook/{id}"))
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(401, response.status().as_u16());
    }

    #[tokio::test]
    async fn test_get_all_webhooks() {
        let (router, db) = get_app().await;

        let mut ids = vec![];
        for _ in 0..5 {
            ids.push(db.register_webhook(sample_webhook()).await.unwrap());
        }

        let response = router
            .clone()
            .oneshot(
                Request::get("/api/v1/webhook")
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(200, response.status().as_u16());
        let response: GetAllWebhooksResponse = parse_response(response).await;

        assert_eq!(5, response.0.len());
    }

    #[tokio::test]
    async fn test_get_all_webhooks_non_admin() {
        let (router, db) = get_app().await;

        let mut ids = vec![];
        for _ in 0..2 {
            ids.push(db.register_webhook(sample_webhook()).await.unwrap());
        }

        let response = router
            .clone()
            .oneshot(
                Request::get("/api/v1/webhook")
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(401, response.status().as_u16());
    }

    #[tokio::test]
    async fn test_delete_webhook() {
        let (router, db) = get_app().await;

        let id = db.register_webhook(sample_webhook()).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/webhook/{id}"))
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(200, response.status().as_u16());

        let result = db.get_webhook(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_webhook_non_admin() {
        let (router, db) = get_app().await;

        let id = db.register_webhook(sample_webhook()).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/webhook/{id}"))
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(401, response.status().as_u16());

        let result = db.get_webhook(&id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_test_webhook() {
        let (router, db) = get_app().await;
        let mut listener = get_test_listener(9977, 200).await;

        let mut webhook = sample_webhook();
        webhook.callback_url = "http://0.0.0.0:9977".to_string();
        let id = db.register_webhook(webhook).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/webhook/{id}/test"))
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(200, response.status().as_u16());

        let listener_resp = listener.rx.recv().await.unwrap();
        assert_eq!(0, listener_resp);
    }

    #[tokio::test]
    async fn test_test_webhook_does_not_return_callback_body() {
        let (router, db) = get_app().await;
        let body = "internal-callback-details";
        let _listener = get_test_listener_with_response(
            9979,
            format!(
                "HTTP/1.1 503\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
        .await;

        let mut webhook = sample_webhook();
        webhook.callback_url = "http://127.0.0.1:9979".to_string();
        let id = db.register_webhook(webhook).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/webhook/{id}/test"))
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(502, response.status().as_u16());
        let response_body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let response_body = String::from_utf8_lossy(&response_body);
        assert!(!response_body.contains(body));
        assert!(response_body.contains("503"));
    }

    #[tokio::test]
    async fn test_test_webhook_returns_502_for_unauthorized_callback() {
        let (router, db) = get_app().await;
        let _listener = get_test_listener(9996, 401).await;
        let mut webhook = sample_webhook();
        webhook.callback_url = "http://127.0.0.1:9996".to_string();
        let id = db.register_webhook(webhook).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/webhook/{id}/test"))
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(502, response.status().as_u16());
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("401"));
    }

    #[tokio::test]
    async fn test_test_webhook_does_not_return_request_error() {
        let (router, db) = get_app().await;

        let mut webhook = sample_webhook();
        webhook.callback_url = "http://127.0.0.1:9976".to_string();
        let id = db.register_webhook(webhook).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/webhook/{id}/test"))
                    .header(header::AUTHORIZATION, ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(500, response.status().as_u16());
        let response_body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let response_body = String::from_utf8_lossy(&response_body);
        assert!(!response_body.contains("127.0.0.1"));
        assert!(response_body.contains("Webhook callback request failed"));
    }

    #[tokio::test]
    async fn test_test_webhook_non_admin() {
        let (router, db) = get_app().await;
        let mut listener = get_test_listener(9978, 200).await;

        let mut webhook = sample_webhook();
        webhook.callback_url = "http://0.0.0.0:9978".to_string();
        let id = db.register_webhook(webhook).await.unwrap();

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/webhook/{id}/test"))
                    .header(header::AUTHORIZATION, NON_ADMIN_TOKEN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(401, response.status().as_u16());

        let listener_resp = listener.rx.try_recv();
        assert!(listener_resp.is_err());
    }

    async fn get_app() -> (Router, Arc<Database>) {
        let con_string = ConString::Sqlite(SqliteConString::new(
            std::path::Path::new(":memory:"),
            "admin",
            Some("token".to_string()),
            kellnr_settings::SessionAge::from_secs(10),
        ));
        let db = Arc::new(Database::new(&con_string, 1).await.unwrap());

        db.add_auth_token(
            "wh_test_admin",
            &PlainToken::from(ADMIN_TOKEN),
            &UserName::new("admin").unwrap(),
        )
        .await
        .unwrap();
        db.add_user(
            &UserName::new("wh_non_admin").unwrap(),
            "na",
            UserFlags::normal(),
        )
        .await
        .unwrap();
        db.add_auth_token(
            "wh_non_admin",
            &PlainToken::from(NON_ADMIN_TOKEN),
            &UserName::new("wh_non_admin").unwrap(),
        )
        .await
        .unwrap();

        let state = AppStateData {
            db: db.clone(),
            ..kellnr_appstate::test_state()
        };

        let routes = Router::new()
            .route("/", get(get_all_webhooks))
            .route("/", post(register_webhook))
            .route("/{id}", get(get_webhook))
            .route("/{id}", delete(delete_webhook))
            .route("/{id}/test", get(test_webhook));

        (
            Router::new()
                .nest("/api/v1/webhook", routes)
                .with_state(state),
            db,
        )
    }

    async fn parse_response<T: DeserializeOwned>(response: Response<Body>) -> T {
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    }

    fn sample_webhook() -> Webhook {
        Webhook {
            id: None,
            event: WebhookEvent::CrateUpdate,
            callback_url: "https://some-callback:8000".to_string(),
            name: Some("My callback".to_string()),
            secret: None,
        }
    }
}
