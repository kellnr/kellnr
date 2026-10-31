//! Which login methods every endpoint accepts.
//!
//! Every endpoint is called without credentials, with a cargo token, with a
//! session cookie, with an invalid token and with an invalid token next to a
//! valid session, once with `auth_required` and once without. The resulting
//! status codes are compared with a fixed table, so a change of the accepted
//! login methods of any endpoint shows up here. A `*` after a status marks a
//! response that reveals the download-restricted crate, i.e. the caller was
//! recognized as its owner.

use std::fmt::Write as _;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, header};
use axum_extra::extract::cookie::Key;
use bytes::Bytes;
use kellnr_common::account_name::UserName;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::plain_token::PlainToken;
use kellnr_common::version::Version;
use kellnr_db::{ConString, Database, DbProvider, SqliteConString};
use kellnr_http::state::AppStateData;
use kellnr_http::test_support::{TEST_KEY, encode_cookies, test_state};
use kellnr_settings::Settings;
use kellnr_settings::constants::COOKIE_SESSION_ID;
use kellnr_storage::docs_storage::{DocsName, DocsStorage};
use kellnr_storage::object_storage::MemoryStorage;
use tower::ServiceExt;

use super::create_router;

const TOKEN: &str = "admin-token";
const SESSION: &str = "admin-session";
const RESTRICTED: &str = "restricted";

/// Method and URI of every endpoint, with parameters that let the request
/// reach the handler where that is cheap.
const ENDPOINTS: &[(&str, &str)] = &[
    // Web UI API
    ("GET", "/api/v1/ui/version"),
    ("GET", "/api/v1/ui/settings"),
    ("GET", "/api/v1/ui/docs_enabled"),
    ("GET", "/api/v1/ui/sponsor_links_enabled"),
    ("GET", "/api/v1/ui/crates"),
    ("GET", "/api/v1/ui/search?name=restricted"),
    ("GET", "/api/v1/ui/crate_data?name=restricted"),
    ("GET", "/api/v1/ui/crate_dependents?name=restricted"),
    ("GET", "/api/v1/ui/statistics"),
    ("DELETE", "/api/v1/ui/crates/missing/1.0.0"),
    ("DELETE", "/api/v1/ui/crates/missing"),
    ("GET", "/api/v1/auth/state"),
    ("GET", "/api/v1/users"),
    ("POST", "/api/v1/users"),
    ("DELETE", "/api/v1/users/missing"),
    ("PUT", "/api/v1/users/missing/password"),
    ("POST", "/api/v1/users/missing/admin"),
    ("POST", "/api/v1/users/missing/read-only"),
    ("PUT", "/api/v1/users/me/password"),
    ("GET", "/api/v1/users/me/tokens"),
    ("POST", "/api/v1/users/me/tokens"),
    ("DELETE", "/api/v1/users/me/tokens/9999"),
    ("GET", "/api/v1/groups"),
    ("POST", "/api/v1/groups"),
    ("DELETE", "/api/v1/groups/missing"),
    ("GET", "/api/v1/groups/missing/members"),
    ("PUT", "/api/v1/groups/missing/members/missing"),
    ("DELETE", "/api/v1/groups/missing/members/missing"),
    ("GET", "/api/v1/acl/restricted"),
    ("PUT", "/api/v1/acl/restricted"),
    ("GET", "/api/v1/docs/builds"),
    ("POST", "/api/v1/docs/builds?package=missing&version=1.0.0"),
    ("POST", "/api/v1/docs/migrate-storage"),
    ("GET", "/api/v1/docs/restricted/latest"),
    ("PUT", "/api/v1/docs/missing/1.0.0"),
    ("GET", "/docs/restricted/1.0.0/doc/restricted/index.html"),
    ("GET", "/api/v1/webhooks"),
    ("POST", "/api/v1/webhooks"),
    ("GET", "/api/v1/webhooks/missing"),
    ("DELETE", "/api/v1/webhooks/missing"),
    ("POST", "/api/v1/webhooks/missing/test"),
    ("GET", "/api/v1/toolchains"),
    (
        "PUT",
        "/api/v1/toolchains?name=rust&version=1.0.0&target=x&date=2024-01-01",
    ),
    ("DELETE", "/api/v1/toolchains/rust/1.0.0"),
    ("DELETE", "/api/v1/toolchains/rust/1.0.0/targets/x"),
    ("GET", "/api/v1/toolchains/channels"),
    ("PUT", "/api/v1/toolchains/channels/stable"),
    ("GET", "/api/v1/toolchains/dist/channel-rust-stable.toml"),
    ("GET", "/api/v1/oauth2/config"),
    ("GET", "/api/v1/health"),
    // Cargo registry API
    ("GET", "/api/v1/crates/config.json"),
    ("GET", "/api/v1/crates/re/st/restricted"),
    ("GET", "/api/v1/crates/2/ab"),
    ("GET", "/api/v1/crates?q=restricted"),
    ("GET", "/api/v1/crates/restricted/owners"),
    ("PUT", "/api/v1/crates/restricted/owners"),
    ("DELETE", "/api/v1/crates/restricted/owners"),
    ("PUT", "/api/v1/crates/restricted/owners/missing"),
    ("DELETE", "/api/v1/crates/restricted/owners/missing"),
    ("GET", "/api/v1/crates/restricted/crate_users"),
    ("PUT", "/api/v1/crates/restricted/crate_users/missing"),
    ("DELETE", "/api/v1/crates/restricted/crate_users/missing"),
    ("GET", "/api/v1/crates/restricted/crate_groups"),
    ("PUT", "/api/v1/crates/restricted/crate_groups/missing"),
    ("DELETE", "/api/v1/crates/restricted/crate_groups/missing"),
    ("GET", "/api/v1/crates/restricted/crate_versions"),
    ("GET", "/api/v1/crates/dl/restricted/1.0.0/download"),
    ("PUT", "/api/v1/crates/new"),
    ("PUT", "/api/v1/crates/new_empty"),
    ("DELETE", "/api/v1/crates/missing/1.0.0/yank"),
    ("PUT", "/api/v1/crates/missing/1.0.0/unyank"),
    // crates.io proxy
    ("GET", "/api/v1/cratesio/config.json"),
    ("GET", "/api/v1/cratesio/se/rd/serde"),
    ("GET", "/api/v1/cratesio?q=serde"),
    ("GET", "/api/v1/cratesio/dl/serde/1.0.0/download"),
];

/// Expected status codes per endpoint: first with `auth_required`, then
/// without. Each group is: no credentials, token, session, invalid token,
/// invalid token with session.
const EXPECTED: &str = "\
GET /api/v1/ui/version | 401 401 200 401 200 | 200 200 200 200 200
GET /api/v1/ui/settings | 401 401 200* 401 200* | 401 401 200* 401 200*
GET /api/v1/ui/docs_enabled | 401 401 200 401 200 | 200 200 200 200 200
GET /api/v1/ui/sponsor_links_enabled | 401 401 200 401 200 | 200 200 200 200 200
GET /api/v1/ui/crates | 401 401 200 401 200 | 200 200 200 200 200
GET /api/v1/ui/search?name=restricted | 401 401 200 401 200 | 200 200 200 200 200
GET /api/v1/ui/crate_data?name=restricted | 401 401 200* 401 200* | 404 200* 200* 404 200*
GET /api/v1/ui/crate_dependents?name=restricted | 401 401 200* 401 200* | 404 200* 200* 404 200*
GET /api/v1/ui/statistics | 401 401 200* 401 200* | 200 200* 200* 200 200*
DELETE /api/v1/ui/crates/missing/1.0.0 | 401 401 404 401 404 | 401 401 404 401 404
DELETE /api/v1/ui/crates/missing | 401 401 404 401 404 | 401 401 404 401 404
GET /api/v1/auth/state | 200 200 200 200 200 | 200 200 200 200 200
GET /api/v1/users | 401 401 200 401 200 | 401 401 200 401 200
POST /api/v1/users | 401 401 415 401 415 | 401 401 415 401 415
DELETE /api/v1/users/missing | 401 401 404 401 404 | 401 401 404 401 404
PUT /api/v1/users/missing/password | 401 401 404 401 404 | 401 401 404 401 404
POST /api/v1/users/missing/admin | 401 401 415 401 415 | 401 401 415 401 415
POST /api/v1/users/missing/read-only | 401 401 415 401 415 | 401 401 415 401 415
PUT /api/v1/users/me/password | 401 401 415 401 415 | 401 401 415 401 415
GET /api/v1/users/me/tokens | 401 401 200 401 200 | 401 401 200 401 200
POST /api/v1/users/me/tokens | 401 401 415 401 415 | 401 401 415 401 415
DELETE /api/v1/users/me/tokens/9999 | 401 401 404 401 404 | 401 401 404 401 404
GET /api/v1/groups | 401 401 200 401 200 | 401 401 200 401 200
POST /api/v1/groups | 401 401 415 401 415 | 401 401 415 401 415
DELETE /api/v1/groups/missing | 401 401 404 401 404 | 401 401 404 401 404
GET /api/v1/groups/missing/members | 401 401 200 401 200 | 401 401 200 401 200
PUT /api/v1/groups/missing/members/missing | 401 401 404 401 404 | 401 401 404 401 404
DELETE /api/v1/groups/missing/members/missing | 401 401 404 401 404 | 401 401 404 401 404
GET /api/v1/acl/restricted | 401 200* 200* 401 200* | 401 200* 200* 401 200*
PUT /api/v1/acl/restricted | 401 401 415 401 415 | 401 401 415 401 415
GET /api/v1/docs/builds | 401 401 200 401 200 | 200 200 200 200 200
POST /api/v1/docs/builds?package=missing&version=1.0.0 | 401 401 400 401 400 | 401 401 400 401 400
POST /api/v1/docs/migrate-storage | 401 401 400 401 400 | 401 401 400 401 400
GET /api/v1/docs/restricted/latest | 401 401 307 401 307 | 307 307 307 307 307
PUT /api/v1/docs/missing/1.0.0 | 401 400 401 403 403 | 401 400 401 403 403
GET /docs/restricted/1.0.0/doc/restricted/index.html | 401 401 200* 401 200* | 404 200* 200* 404 200*
GET /api/v1/webhooks | 401 200 401 403 403 | 401 200 401 403 403
POST /api/v1/webhooks | 401 415 401 403 403 | 401 415 401 403 403
GET /api/v1/webhooks/missing | 401 400 401 403 403 | 401 400 401 403 403
DELETE /api/v1/webhooks/missing | 401 400 401 403 403 | 401 400 401 403 403
POST /api/v1/webhooks/missing/test | 401 400 401 403 403 | 401 400 401 403 403
GET /api/v1/toolchains | 401 401 200 401 200 | 401 401 200 401 200
PUT /api/v1/toolchains?name=rust&version=1.0.0&target=x&date=2024-01-01 | 401 401 503 401 503 | 401 401 503 401 503
DELETE /api/v1/toolchains/rust/1.0.0 | 401 401 503 401 503 | 401 401 503 401 503
DELETE /api/v1/toolchains/rust/1.0.0/targets/x | 401 401 503 401 503 | 401 401 503 401 503
GET /api/v1/toolchains/channels | 401 401 200 401 200 | 401 401 200 401 200
PUT /api/v1/toolchains/channels/stable | 401 401 415 401 415 | 401 401 415 401 415
GET /api/v1/toolchains/dist/channel-rust-stable.toml | 404 404 404 404 404 | 404 404 404 404 404
GET /api/v1/oauth2/config | 200 200 200 200 200 | 200 200 200 200 200
GET /api/v1/health | 200 200 200 200 200 | 200 200 200 200 200
GET /api/v1/crates/config.json | 401 200 200 401 200 | 200 200 200 200 200
GET /api/v1/crates/re/st/restricted | 401 200 200 401 200 | 404 200 200 404 200
GET /api/v1/crates/2/ab | 401 404 404 401 404 | 404 404 404 404 404
GET /api/v1/crates?q=restricted | 401 200 200 401 200 | 200 200 200 200 200
GET /api/v1/crates/restricted/owners | 401 200 200 401 200 | 404 200 200 404 200
PUT /api/v1/crates/restricted/owners | 401 415 415 401 415 | 401 415 415 401 415
DELETE /api/v1/crates/restricted/owners | 401 415 415 401 415 | 401 415 415 401 415
PUT /api/v1/crates/restricted/owners/missing | 401 404 404 401 404 | 401 404 404 401 404
DELETE /api/v1/crates/restricted/owners/missing | 401 404 404 401 404 | 401 404 404 401 404
GET /api/v1/crates/restricted/crate_users | 401 200 200 401 200 | 401 200 200 401 200
PUT /api/v1/crates/restricted/crate_users/missing | 401 404 404 401 404 | 401 404 404 401 404
DELETE /api/v1/crates/restricted/crate_users/missing | 401 404 404 401 404 | 401 404 404 401 404
GET /api/v1/crates/restricted/crate_groups | 401 200 200 401 200 | 401 200 200 401 200
PUT /api/v1/crates/restricted/crate_groups/missing | 401 404 404 401 404 | 401 404 404 401 404
DELETE /api/v1/crates/restricted/crate_groups/missing | 401 404 404 401 404 | 401 404 404 401 404
GET /api/v1/crates/restricted/crate_versions | 401 200 200 401 200 | 404 200 200 404 200
GET /api/v1/crates/dl/restricted/1.0.0/download | 401 404 404 401 403 | 404 404 404 403 403
PUT /api/v1/crates/new | 401 400 401 401 403 | 401 400 401 403 403
PUT /api/v1/crates/new_empty | 401 415 401 401 403 | 401 415 401 403 403
DELETE /api/v1/crates/missing/1.0.0/yank | 401 404 401 401 403 | 401 404 401 403 403
PUT /api/v1/crates/missing/1.0.0/unyank | 401 404 401 401 403 | 401 404 401 403 403
GET /api/v1/cratesio/config.json | 401 404 401 403 403 | 404 404 404 404 404
GET /api/v1/cratesio/se/rd/serde | 401 404 401 403 403 | 404 404 404 404 404
GET /api/v1/cratesio?q=serde | 401 404 401 403 403 | 404 404 404 404 404
GET /api/v1/cratesio/dl/serde/1.0.0/download | 401 404 401 403 403 | 404 404 404 404 404
";

#[derive(Clone, Copy)]
enum Credentials {
    None,
    Token,
    Session,
    InvalidToken,
    InvalidTokenWithSession,
}

const ALL_CREDENTIALS: [Credentials; 5] = [
    Credentials::None,
    Credentials::Token,
    Credentials::Session,
    Credentials::InvalidToken,
    Credentials::InvalidTokenWithSession,
];

struct TestApp {
    _dir: tempfile::TempDir,
    db: Arc<Database>,
    state: AppStateData,
}

impl TestApp {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = Settings {
            registry: kellnr_settings::Registry {
                data_dir: dir.path().to_str().unwrap().to_string(),
                ..kellnr_settings::Registry::default()
            },
            ..Settings::default()
        };
        settings.toolchain.enabled = true;
        let con = ConString::Sqlite(SqliteConString::from(&settings));
        kellnr_test_db::copy_sqlite_template(&con).await;
        let db = Arc::new(Database::new(&con, 10).await.unwrap());
        let admin = UserName::new("admin").unwrap();
        db.add_auth_token("test", &PlainToken::from(TOKEN), &admin)
            .await
            .unwrap();
        let crate_id = db.add_empty_crate(RESTRICTED, &chrono::Utc::now()).await;
        crate_id.unwrap();
        let name = CanonicalName::try_from(RESTRICTED).unwrap();
        db.add_owner(&name, &admin).await.unwrap();
        db.change_download_restricted(&name, true).await.unwrap();

        let docs_storage = DocsStorage::new(Box::new(MemoryStorage::new()));
        let key = DocsStorage::file_key(
            &DocsName::from_folder(RESTRICTED.to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "doc/restricted/index.html",
        );
        docs_storage
            .put(&key, Bytes::from_static(b"<html>restricted</html>"))
            .await
            .unwrap();

        let dyn_db: Arc<dyn DbProvider> = db.clone();
        let state = AppStateData {
            db: dyn_db,
            signing_key: Key::from(TEST_KEY),
            settings: Arc::new(settings),
            docs_storage: Arc::new(docs_storage),
            ..test_state()
        };
        Self {
            _dir: dir,
            db,
            state,
        }
    }

    /// Status of the request, with a `*` if the response names the
    /// restricted crate.
    async fn call(
        &self,
        method: &str,
        uri: &str,
        auth_required: bool,
        credentials: Credentials,
    ) -> String {
        // Logout ends the session, so every request gets a fresh one.
        let _ = self
            .db
            .add_session_token(&UserName::new("admin").unwrap(), &PlainToken::from(SESSION))
            .await;
        let mut settings = Settings::clone(&self.state.settings);
        settings.registry.auth_required = auth_required;
        let state = AppStateData {
            settings: Arc::new(settings),
            ..self.state.clone()
        };

        let mut request = Request::builder()
            .method(Method::from_bytes(method.as_bytes()).unwrap())
            .uri(uri);
        let cookie = encode_cookies([(COOKIE_SESSION_ID, SESSION)]);
        request = match credentials {
            Credentials::None => request,
            Credentials::Token => request.header(header::AUTHORIZATION, TOKEN),
            Credentials::Session => request.header(header::COOKIE, cookie),
            Credentials::InvalidToken => request.header(header::AUTHORIZATION, "invalid"),
            Credentials::InvalidTokenWithSession => request
                .header(header::AUTHORIZATION, "invalid")
                .header(header::COOKIE, cookie),
        };
        let response = create_router(state, None)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let reveals = status.is_success() && String::from_utf8_lossy(&body).contains(RESTRICTED);
        format!("{}{}", status.as_u16(), if reveals { "*" } else { "" })
    }
}

#[tokio::test]
async fn endpoints_accept_the_same_login_methods() {
    let mut actual = String::new();
    for (method, uri) in ENDPOINTS {
        let app = TestApp::new().await;
        let mut groups = Vec::new();
        for auth_required in [true, false] {
            let mut statuses = Vec::new();
            for credentials in ALL_CREDENTIALS {
                statuses.push(app.call(method, uri, auth_required, credentials).await);
            }
            groups.push(statuses.join(" "));
        }
        writeln!(actual, "{method} {uri} | {}", groups.join(" | ")).unwrap();
    }

    assert_eq!(EXPECTED, actual, "actual table:\n{actual}");
}
