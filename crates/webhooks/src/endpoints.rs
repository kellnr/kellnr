use appstate::DbState;
use auth::token;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
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
            event: input.event,
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
        event: w.event,
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
        .map_err(|e| ApiError::new(&e.to_string(), "", StatusCode::INTERNAL_SERVER_ERROR))?;

    match resp.status() {
        a if a.as_u16() < 300 => Ok(()),
        a => Err(ApiError::new(&resp.text().await.unwrap_or_default(), "", a)),
    }
}

#[cfg(test)]
mod endpoint_tests {
    use appstate::AppStateData;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use axum::response::Response;
    use axum::routing::{delete, get, post};
    use axum::Router;
    use common::webhook::WebhookEvent;
    use db::{ConString, Database, DbProvider, SqliteConString};
    use hyper::header;
    use serde::de::DeserializeOwned;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::tests::get_test_listener;
    use crate::types::{GetAllWebhooksResponse, GetWebhookResponse, RegisterWebhookResponse};

    use super::*;

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
            "salt",
            "admin",
            "token",
            std::time::Duration::from_secs(10),
        ));
        let db = Arc::new(Database::new(&con_string, 1).await.unwrap());

        db.add_auth_token("wh_test_admin", ADMIN_TOKEN, "admin")
            .await
            .unwrap();
        db.add_user("wh_non_admin", "na", "", false, false)
            .await
            .unwrap();
        db.add_auth_token("wh_non_admin", NON_ADMIN_TOKEN, "wh_non_admin")
            .await
            .unwrap();

        let state = AppStateData {
            db: db.clone(),
            ..appstate::test_state()
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
        }
    }
}
