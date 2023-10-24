use super::config_json::ConfigJson;
use auth::auth_req_token::AuthReqToken;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::prefetch::{Headers, Prefetch};
use db::DbProvider;
use rocket::http::Status;
use rocket::response::status;
use rocket::State;
use settings::Settings;

// #[get("/config.json")]
pub async fn config_kellnr(settings: &State<Settings>, auth_req_token: AuthReqToken) -> ConfigJson {
    _ = auth_req_token;
    ConfigJson::from((settings.inner(), "crates"))
}

// #[get("/<_>/<_>/<package>", rank = 1)]
pub async fn prefetch_kellnr(
    package: OriginalName,
    headers: Headers,
    auth_req_token: AuthReqToken,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Prefetch, status::Custom<&'static str>> {
    _ = auth_req_token;
    let index_name = NormalizedName::from(package);
    internal_kellnr_prefetch(&index_name, &headers, db).await
}

// #[get("/<_>/<package>", rank = 1)]
pub async fn prefetch_len2_kellnr(
    package: OriginalName,
    headers: Headers,
    auth_req_token: AuthReqToken,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Prefetch, status::Custom<&'static str>> {
    _ = auth_req_token;
    let index_name = NormalizedName::from(package);
    internal_kellnr_prefetch(&index_name, &headers, db).await
}

async fn internal_kellnr_prefetch(
    name: &NormalizedName,
    headers: &Headers,
    db: &State<Box<dyn DbProvider>>,
) -> Result<Prefetch, status::Custom<&'static str>> {
    match db.get_prefetch_data(name).await {
        Ok(prefetch) if needs_update(headers, &prefetch) => Ok(prefetch),
        Ok(_prefetch) => Err(status::Custom(Status::NotModified, "Index up-to-date")),
        Err(_) => Err(status::Custom(Status::NotFound, "Index not found")),
    }
}

fn needs_update(headers: &Headers, prefetch: &Prefetch) -> bool {
    match (&headers.if_none_match, &headers.if_modified_since) {
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
