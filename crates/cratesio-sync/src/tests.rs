use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderMap, StatusCode as HttpStatus, header};
use axum::response::IntoResponse;
use axum::routing::get;
use kellnr_common::version::Version;
use kellnr_db::CrateVisibility;
use kellnr_db::mock::MockDb;
use kellnr_test_db::TestDb;
use reqwest::Url;

use super::*;

const INDEX: &str = concat!(
    r#"{"name":"upstream","vers":"1.0.0","deps":[],"cksum":"aaa","features":{},"yanked":false}"#,
    "\n",
    r#"{"name":"upstream","vers":"1.1.0","deps":[],"cksum":"bbb","features":{},"yanked":false}"#,
);

const API: &str = r#"{
    "crate": {"description": "An upstream crate"},
    "versions": [
        {"num": "1.1.0", "license": null},
        {"num": "1.0.0", "license": "MIT"}
    ]
}"#;

/// A sync that waits only briefly between tries, so failing upstreams do not
/// slow the tests down.
fn test_sync(
    db: Arc<dyn DbProvider>,
    client: Client,
    proxy: &kellnr_settings::Proxy,
) -> CratesioSync {
    CratesioSync::with_retry_base_delay(db, client, proxy, Duration::from_millis(10))
}

#[derive(Default)]
struct Calls {
    index: AtomicUsize,
    api: AtomicUsize,
}

/// A fake crates.io with a sparse index and an API. `api_ok` false makes the
/// API fail. The index answers slowly, so concurrent syncs overlap.
async fn fake_cratesio(api_ok: bool, calls: Arc<Calls>) -> kellnr_settings::Proxy {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let index_calls = calls.clone();
    let app = Router::new()
        .route(
            "/index/{a}/{b}/{name}",
            get(move |headers: HeaderMap| {
                let calls = index_calls.clone();
                async move {
                    calls.index.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    if headers
                        .get(header::IF_NONE_MATCH)
                        .is_some_and(|etag| etag == "etag")
                    {
                        return HttpStatus::NOT_MODIFIED.into_response();
                    }
                    ([(header::ETAG, "etag")], INDEX).into_response()
                }
            }),
        )
        .route(
            "/api/{name}",
            get(move || {
                let calls = calls.clone();
                async move {
                    calls.api.fetch_add(1, Ordering::SeqCst);
                    if api_ok {
                        API.into_response()
                    } else {
                        HttpStatus::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    kellnr_settings::Proxy {
        index: Url::parse(&format!("http://{addr}/index/")).unwrap(),
        api: Url::parse(&format!("http://{addr}/api/")).unwrap(),
        ..kellnr_settings::Proxy::default()
    }
}

/// What the database holds of "upstream": its description and the license
/// of each version, `None` if the crate is not stored.
async fn stored(db: &TestDb) -> Option<(Option<String>, HashMap<String, Option<String>>)> {
    let krate = db
        .search_in_crate_name_and_description(
            "upstream",
            10,
            0,
            true,
            &CrateVisibility::Unrestricted,
        )
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.is_cache)?;
    let mut licenses = HashMap::new();
    for vers in ["1.0.0", "1.1.0"] {
        let meta = db
            .get_cratesio_version_meta(
                &upstream().to_canonical(),
                &Version::try_from(vers).unwrap(),
            )
            .await
            .unwrap();
        if let Some(meta) = meta {
            licenses.insert(vers.to_string(), meta.license);
        }
    }
    Some((krate.description, licenses))
}

/// Stores the index of "upstream" with the entity tag `etag` and the
/// description "stored".
async fn store_upstream(db: &TestDb, etag: &str) {
    let indices: Vec<IndexMetadata> = INDEX
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    db.add_cratesio_prefetch_data(
        &upstream(),
        etag,
        "",
        Some("stored".to_string()),
        &indices,
        &HashMap::new(),
    )
    .await
    .unwrap();
}

fn upstream() -> OriginalName {
    OriginalName::try_from("upstream").unwrap()
}

#[tokio::test]
async fn fetch_stores_index_and_licenses() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    let synced = sync.sync(&upstream(), SyncSource::Fetch).await;

    assert_eq!(2, synced.written().unwrap().len());
    let (description, licenses) = stored(&db).await.unwrap();
    assert_eq!(Some("An upstream crate".to_string()), description);
    assert_eq!(2, licenses.len());
    assert_eq!(Some(&Some("MIT".to_string())), licenses.get("1.0.0"));
    assert_eq!(
        Some(&Some(UNKNOWN_LICENSE.to_string())),
        licenses.get("1.1.0")
    );
}

#[tokio::test]
async fn api_failure_still_stores_index() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(false, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    let synced = sync.sync(&upstream(), SyncSource::Fetch).await;

    assert!(matches!(synced, SyncOutcome::Written(_)));
    let (description, licenses) = stored(&db).await.unwrap();
    assert_eq!(None, description);
    assert_eq!(2, licenses.len());
    assert!(licenses.values().all(Option::is_none));
}

#[tokio::test]
async fn concurrent_syncs_of_a_crate_share_one_job() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    // E.g. a download that misses the checksum while the prefetch worker
    // refreshes the same crate
    let name = upstream();
    let (a, b) = tokio::join!(
        sync.sync(&name, SyncSource::Fetch),
        sync.sync(
            &name,
            SyncSource::Refresh {
                etag: None,
                last_modified: None
            }
        ),
    );

    assert!(matches!(
        (a, b),
        (SyncOutcome::Written(_), SyncOutcome::Written(_))
    ));
    assert_eq!(1, calls.index.load(Ordering::SeqCst));
    assert_eq!(1, calls.api.load(Ordering::SeqCst));
    assert!(stored(&db).await.is_some());
    assert!(sync.inner.running.lock().unwrap().is_empty());
}

#[tokio::test]
async fn finished_sync_is_not_cached() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    sync.sync(&upstream(), SyncSource::Fetch).await;
    sync.sync(&upstream(), SyncSource::Fetch).await;

    assert_eq!(2, calls.index.load(Ordering::SeqCst));
    assert_eq!(2, calls.api.load(Ordering::SeqCst));
}

#[tokio::test]
async fn index_source_skips_stored_index() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    store_upstream(&db, "etag").await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    let synced = sync
        .sync(
            &upstream(),
            SyncSource::Index {
                etag: Some("etag".to_string()),
                last_modified: None,
                data: INDEX.to_string(),
            },
        )
        .await;

    assert!(matches!(synced, SyncOutcome::Unchanged));
    assert_eq!(0, calls.api.load(Ordering::SeqCst));
    let (description, _) = stored(&db).await.unwrap();
    assert_eq!(Some("stored".to_string()), description);
}

#[tokio::test]
async fn index_source_stores_new_index_without_fetching_it() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    let synced = sync
        .sync(
            &upstream(),
            SyncSource::Index {
                etag: Some("etag".to_string()),
                last_modified: None,
                data: INDEX.to_string(),
            },
        )
        .await;

    assert!(matches!(synced, SyncOutcome::Written(_)));
    assert_eq!(0, calls.index.load(Ordering::SeqCst));
    assert_eq!(1, calls.api.load(Ordering::SeqCst));
    assert!(stored(&db).await.is_some());
}

#[tokio::test]
async fn refresh_of_unchanged_index_writes_nothing() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    let synced = sync
        .sync(
            &upstream(),
            SyncSource::Refresh {
                etag: Some("etag".to_string()),
                last_modified: None,
            },
        )
        .await;

    assert!(matches!(synced, SyncOutcome::Unchanged));
    assert_eq!(1, calls.index.load(Ordering::SeqCst));
    assert_eq!(0, calls.api.load(Ordering::SeqCst));
    assert!(stored(&db).await.is_none());
}

#[tokio::test]
async fn refresh_of_changed_index_writes_it() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    let synced = sync
        .sync(
            &upstream(),
            SyncSource::Refresh {
                etag: Some("old-etag".to_string()),
                last_modified: None,
            },
        )
        .await;

    assert_eq!(2, synced.written().unwrap().len());
    assert_eq!(1, calls.api.load(Ordering::SeqCst));
    assert!(stored(&db).await.is_some());
}

#[tokio::test]
async fn crate_missing_upstream_is_not_found() {
    let calls = Arc::new(Calls::default());
    let proxy = fake_cratesio(true, calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    // The fake index only serves crates with more than three characters.
    let synced = sync
        .sync(&OriginalName::try_from("ab").unwrap(), SyncSource::Fetch)
        .await;

    assert!(matches!(synced, SyncOutcome::NotFound));
    assert!(stored(&db).await.is_none());
}

#[tokio::test]
async fn invalid_index_url_fails() {
    let proxy = kellnr_settings::Proxy {
        index: Url::parse("ftp://index.example.com/").unwrap(),
        ..kellnr_settings::Proxy::default()
    };
    let sync = test_sync(Arc::new(MockDb::new()), Client::new(), &proxy);

    let synced = sync.sync(&upstream(), SyncSource::Fetch).await;

    assert!(matches!(synced, SyncOutcome::Failed));
}

#[tokio::test]
async fn panicking_sync_does_not_block_later_syncs() {
    let mut db = MockDb::new();
    db.expect_is_cratesio_cache_up_to_date()
        .returning(|_, _, _| panic!("database failure"));
    let sync = test_sync(
        Arc::new(db),
        Client::new(),
        &kellnr_settings::Proxy::default(),
    );

    let synced = sync
        .sync(
            &upstream(),
            SyncSource::Index {
                etag: None,
                last_modified: None,
                data: INDEX.to_string(),
            },
        )
        .await;

    // A later sync of the crate starts a new job instead of joining this one
    assert!(matches!(synced, SyncOutcome::Failed));
    assert!(sync.inner.running.lock().unwrap().is_empty());
}

#[test]
fn crate_sub_path_follows_sparse_index_layout() {
    let path = |n: &str| crate_sub_path(&OriginalName::try_from(n).unwrap());

    assert_eq!("1/a", path("a"));
    assert_eq!("2/ab", path("ab"));
    assert_eq!("3/a/abc", path("abc"));
    assert_eq!("ab/cd/abcd", path("abcd"));
    assert_eq!("fo/o-/foo-bar", path("Foo-Bar"));
    assert_eq!("fo/o_/foo_bar", path("foo_bar"));
}

/// A fake crates.io whose index answers 503 and whose API answers 429 to the
/// first request.
async fn flaky_cratesio(calls: Arc<Calls>) -> kellnr_settings::Proxy {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let index_calls = calls.clone();
    let app = Router::new()
        .route(
            "/index/{a}/{b}/{name}",
            get(move || {
                let calls = index_calls.clone();
                async move {
                    if calls.index.fetch_add(1, Ordering::SeqCst) == 0 {
                        HttpStatus::SERVICE_UNAVAILABLE.into_response()
                    } else {
                        INDEX.into_response()
                    }
                }
            }),
        )
        .route(
            "/api/{name}",
            get(move || {
                let calls = calls.clone();
                async move {
                    if calls.api.fetch_add(1, Ordering::SeqCst) == 0 {
                        (HttpStatus::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")])
                            .into_response()
                    } else {
                        API.into_response()
                    }
                }
            }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    kellnr_settings::Proxy {
        index: Url::parse(&format!("http://{addr}/index/")).unwrap(),
        api: Url::parse(&format!("http://{addr}/api/")).unwrap(),
        ..kellnr_settings::Proxy::default()
    }
}

#[tokio::test]
async fn server_errors_and_rate_limits_are_retried() {
    let calls = Arc::new(Calls::default());
    let proxy = flaky_cratesio(calls.clone()).await;
    let db = TestDb::sqlite().await;
    let sync = test_sync(db.provider(), Client::new(), &proxy);

    let synced = sync.sync(&upstream(), SyncSource::Fetch).await;

    assert_eq!(2, synced.written().unwrap().len());
    assert_eq!(2, calls.index.load(Ordering::SeqCst));
    assert_eq!(2, calls.api.load(Ordering::SeqCst));
    let (description, _) = stored(&db).await.unwrap();
    assert_eq!(Some("An upstream crate".to_string()), description);
}

#[tokio::test]
async fn retries_wait_with_backoff() {
    let tries = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/",
        get({
            let tries = tries.clone();
            move || async move {
                tries.fetch_add(1, Ordering::SeqCst);
                HttpStatus::BAD_GATEWAY
            }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::new();
    let start = std::time::Instant::now();

    let response = send_with_retries("test", Duration::from_millis(50), || {
        client.get(format!("http://{addr}/"))
    })
    .await
    .unwrap();

    assert_eq!(StatusCode::BAD_GATEWAY, response.status());
    assert_eq!(MAX_TRIES as usize, tries.load(Ordering::SeqCst));
    // 50ms before the second and 100ms before the third try
    assert!(start.elapsed() >= Duration::from_millis(150));
}
