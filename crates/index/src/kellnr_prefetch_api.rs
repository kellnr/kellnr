use std::sync::Arc;

use super::config_json::ConfigJson;
use appstate::{DbState, SettingsState};
use auth::auth_req_token::AuthReqToken;
use axum::{extract::{Path, State}, http::{StatusCode, HeaderMap}, Json};
use common::{normalized_name::NormalizedName, original_name::OriginalName, prefetch::Prefetch};
use db::DbProvider;

pub async fn config_kellnr(
    State(settings): SettingsState,
    auth_req_token: AuthReqToken,
) -> Json<ConfigJson> {
    _ = auth_req_token;
    Json(ConfigJson::from((&(*settings), "crates")))
}

pub async fn prefetch_kellnr(
    Path(package): Path<OriginalName>,
    headers: HeaderMap,
    State(db): DbState,
    auth_req_token: AuthReqToken,
) -> Result<Prefetch, StatusCode> {
    _ = auth_req_token;
    let index_name = NormalizedName::from(package);
    internal_kellnr_prefetch(&index_name, &headers, &db).await
}

pub async fn prefetch_len2_kellnr(
    Path(package): Path<OriginalName>,
    headers: HeaderMap,
    auth_req_token: AuthReqToken,
    State(db): DbState,
) -> Result<Prefetch, StatusCode> {
    _ = auth_req_token;
    let index_name = NormalizedName::from(package);
    internal_kellnr_prefetch(&index_name, &headers, &db).await
}

async fn internal_kellnr_prefetch(
    name: &NormalizedName,
    headers: &HeaderMap,
    db: &Arc<dyn DbProvider>,
) -> Result<Prefetch, StatusCode> {
    match db.get_prefetch_data(name).await {
        Ok(prefetch) if needs_update(headers, &prefetch) => Ok(prefetch),
        Ok(_prefetch) => Err(StatusCode::NOT_MODIFIED),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

fn needs_update(headers: &HeaderMap, prefetch: &Prefetch) -> bool {
    let if_none_match = headers.get("if-none-match");
    let if_modified_since = headers.get("if-modified-since");

    match (if_none_match, if_modified_since) {
        (Some(etag), Some(date)) => *etag != prefetch.etag || *date != prefetch.last_modified,
        (_, _) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_json::ConfigJson;
    use db::error::DbError;
    use db::mock::MockDb;
    use mockall::predicate::*;
    use rocket::config::{Config, SecretKey};
    use rocket::http::{Header, Status};
    use rocket::local::blocking::Client;
    use rocket::routes;
    use settings::{Protocol, Settings};

    #[test]
    fn config_returns_config_json() {
        let client = client();
        let req = client.get("/api/v1/index/config.json");
        let result = req.dispatch();
        let result_msg = result.into_string().unwrap();
        let actual = serde_json::from_str::<ConfigJson>(&result_msg).unwrap();

        assert_eq!(
            ConfigJson::new(&Protocol::Http, "test.api.com", 1234, "crates", false),
            actual
        );
    }

    #[test]
    fn prefetch_returns_prefetch_data() {
        let client = client();
        let req = client
            .get("/api/v1/index/me/ta/metadata")
            .header(Header::new("If-Modified-Since", "foo"))
            .header(Header::new("ETag", "bar"));
        let result = req.dispatch();
        let result_status = result.status();

        assert_eq!(Status::Ok, result_status);
        assert_eq!("3", result.headers().get_one("Content-Length").unwrap());
        assert_eq!("date", result.headers().get_one("Last-Modified").unwrap());
        assert_eq!(vec![0x1, 0x2, 0x3], result.into_bytes().unwrap());
    }

    #[test]
    fn prefetch_returns_not_modified() {
        let client = client();
        let req = client
            .get("/api/v1/index/me/ta/metadata")
            .header(Header::new("If-Modified-Since", "date"))
            .header(Header::new("If-None-Match", "etag"));
        let result = req.dispatch();

        assert_eq!(Status::NotModified, result.status());
    }

    #[test]
    fn prefetch_returns_not_found() {
        let client = client();
        let req = client
            .get("/api/v1/index/no/tf/notfound")
            .header(Header::new("If-Modified-Since", "date"))
            .header(Header::new("If-None-Match", "etag"));
        let result = req.dispatch();

        assert_eq!(Status::NotFound, result.status());
    }

    fn client() -> Client {
        let settings = Settings {
            api_address: String::from("test.api.com"),
            api_port: 8000,
            api_port_proxy: 1234,
            ..Settings::new().unwrap()
        };

        let mut mock_db = MockDb::new();
        mock_db
            .expect_get_prefetch_data()
            .with(eq("metadata"))
            .returning(move |_| {
                Ok(Prefetch {
                    data: vec![0x1, 0x2, 0x3],
                    etag: String::from("etag"),
                    last_modified: String::from("date"),
                })
            });
        mock_db
            .expect_get_prefetch_data()
            .with(eq("notfound"))
            .returning(move |_| Err(DbError::CrateNotFound("notfound".to_string())));

        let db = Box::new(mock_db) as Box<dyn DbProvider>;

        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        let rocket = rocket::custom(rocket_conf)
            .mount(
                "/api/v1/index",
                routes![config_kellnr, prefetch_kellnr, prefetch_len2_kellnr],
            )
            .manage(db)
            .manage(settings);

        Client::tracked(rocket).unwrap()
    }
}
