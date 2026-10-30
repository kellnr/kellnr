use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use kellnr_common::crate_data::{CrateData, CrateRegistryDep, CrateVersionData};
use kellnr_common::crate_overview::CrateOverview;
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::license::UNKNOWN_LICENSE;
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::Prefetch;
use kellnr_common::publish_metadata::{PublishMetadata, RegistryDep};
use kellnr_common::version::Version;
use kellnr_common::webhook::{Webhook, WebhookEvent};
use kellnr_db::provider::PrefetchState;
use kellnr_db::test_utils::*;
use kellnr_db::{CrateSort, CrateTagFilter, CrateVisibility, DbProvider, DocQueueEntry};
use kellnr_db_testcontainer::db_test;
use serde_json::json;
mod image;

#[db_test]
async fn get_total_unique_crates_returns_number_of_unique_crates(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate2",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();

    let total_versions = test_db.get_total_crate_versions().await.unwrap();

    assert_eq!(3, total_versions);
}

#[db_test]
async fn get_total_crate_versions_returns_number_of_crate_versions(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate2",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();

    let total_versions = test_db.get_total_crate_versions().await.unwrap();

    assert_eq!(3, total_versions);
}

#[db_test]
async fn get_total_downloads_returns_number_of_total_downloads(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let id1 = test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    let id2 = test_add_crate(
        test_db,
        "crate2",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate_meta(
        test_db,
        id1,
        &Version::try_from("1.1.0").unwrap(),
        &created,
        None,
    )
    .await
    .unwrap();
    test_add_crate_meta(
        test_db,
        id1,
        &Version::try_from("2.0.0").unwrap(),
        &created,
        None,
    )
    .await
    .unwrap();
    test_add_crate_meta(
        test_db,
        id2,
        &Version::try_from("1.1.0").unwrap(),
        &created,
        None,
    )
    .await
    .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("2.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate2"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    let total_downloads = test_db.get_total_downloads().await.unwrap();

    assert_eq!(4, total_downloads);
}

#[db_test]
async fn get_top_crates_downloads_returns_top_crates_with_downloads(test_db: &kellnr_db::Database) {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let crate_id = test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();

    test_add_crate_meta(
        test_db,
        crate_id,
        &Version::try_from("0.1.0").unwrap(),
        &created1,
        None,
    )
    .await
    .unwrap();

    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("mycrate"),
            &Version::from_unchecked_str("0.1.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("mycrate"),
            &Version::from_unchecked_str("0.1.0"),
        )
        .await
        .unwrap();
}

#[db_test]
async fn increase_download_counter_works(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate2",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate3",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate2",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate3",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();

    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate1"),
            &Version::from_unchecked_str("2.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate2"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate3"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    test_db
        .increase_download_counter(
            &NormalizedName::from_unchecked_str("crate3"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    let tops = test_db
        .get_top_crates_downloads(2, &CrateVisibility::All)
        .await
        .unwrap();
    assert_eq!(2, tops.len());
    assert_eq!(("crate1".to_string(), 3), tops[0]);
    assert_eq!(("crate3".to_string(), 2), tops[1]);
}

#[db_test]
async fn get_max_version_from_id(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    let crate_id1 = test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("0.1.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("0.2.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("0.10.0").unwrap(),
        &created2,
    )
    .await
    .unwrap();

    let version = test_db.get_max_version_from_id(crate_id1).await.unwrap();

    assert_eq!("0.10.0", version.to_string());
}

#[db_test]
async fn get_max_version_from_name(test_db: &kellnr_db::Database) {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("0.1.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("0.2.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("0.10.0").unwrap(),
        &created2,
    )
    .await
    .unwrap();

    let version = test_db
        .get_max_version_from_name(&NormalizedName::from_unchecked("acrate".to_string()))
        .await
        .unwrap();

    assert_eq!("0.10.0", version.to_string());
}

#[db_test]
async fn get_crate_summaries_works(test_db: &kellnr_db::Database) {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 11, 22, 12).unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("1.1.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "bcrate",
        "admin",
        &Version::try_from("1.1.0").unwrap(),
        &created2,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("1.2.0").unwrap(),
        &created2,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "bcrate",
        "admin",
        &Version::try_from("1.2.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();

    let crates = test_db.get_crate_summaries().await.unwrap();

    assert_eq!(2, crates.len());
    assert_eq!("acrate", crates[0].name);
    assert_eq!("1.2.0", crates[0].max_version);
    assert_eq!(0, crates[0].total_downloads);
    assert_eq!("2020-10-08 11:22:12", crates[0].last_updated);
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 11, 22, 12).unwrap();
    test_add_crate(
        test_db,
        "acrate",
        "admin",
        &Version::try_from("1.3.0").unwrap(),
        &created2,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "bcrate",
        "admin",
        &Version::try_from("1.3.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();

    let crates = test_db.get_crate_summaries().await.unwrap();

    assert_eq!(2, crates.len());
    assert_eq!("acrate", crates[0].name);
    assert_eq!("1.3.0", crates[0].max_version);
    assert_eq!(0, crates[0].total_downloads);
    assert_eq!("2020-10-08 11:22:12", crates[0].last_updated);

    assert_eq!("bcrate", crates[1].name);
    assert_eq!("1.3.0", crates[1].max_version);
    assert_eq!(0, crates[1].total_downloads);
    assert_eq!("2020-10-07 13:18:00", crates[1].last_updated);
}

#[db_test]
async fn get_crate_versions_returns_all_versions(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate1",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate2",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();

    let versions = test_db
        .get_crate_versions(&NormalizedName::from_unchecked_str("crate1"))
        .await
        .unwrap();

    let expected = vec![
        Version::try_from("1.0.0").unwrap(),
        Version::try_from("2.0.0").unwrap(),
    ];
    assert_eq!(expected, versions);
}

#[db_test]
async fn get_crate_versions_with_yanked_version(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();

    // Yank crate version 2.0.0
    test_db
        .yank_crate(
            &NormalizedName::from_unchecked_str("crate"),
            &Version::from_unchecked_str("2.0.0"),
        )
        .await
        .unwrap();

    let versions = test_db
        .get_crate_versions(&NormalizedName::from_unchecked_str("crate"))
        .await
        .unwrap();

    let expected = vec![
        Version::try_from("1.0.0").unwrap(),
        Version::try_from("2.0.0").unwrap(),
    ];
    assert_eq!(expected, versions);
}

#[db_test]
async fn get_crate_versions_for_nonexistant_crate(test_db: &kellnr_db::Database) {
    let outcome = test_db
        .get_crate_versions(&NormalizedName::from_unchecked_str("crate1"))
        .await
        .unwrap();

    assert_eq!(outcome, vec![]);
}

#[db_test]
async fn test_add_crate_meta_and_read_meta(test_db: &kellnr_db::Database) {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 11, 22, 12).unwrap();
    let crate_id = test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("0.1.0").unwrap(),
        &created1,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("0.1.1").unwrap(),
        &created2,
    )
    .await
    .unwrap();
    let meta = get_crate_meta_list(test_db, crate_id).await.unwrap();

    assert_eq!(2, meta.len());
    assert_eq!("0.1.0", meta[0].version);
    assert_eq!("0.1.1", meta[1].version);
    assert_eq!(0, meta[0].downloads);
    assert_eq!(0, meta[1].downloads);
    assert_eq!("2020-10-07 13:18:00", meta[0].created);
    assert_eq!("2020-10-08 11:22:12", meta[1].created);
}

#[db_test]
async fn is_owner_true(test_db: &kellnr_db::Database) {
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    assert!(
        test_db
            .is_owner(
                &NormalizedName::from_unchecked("mycrate".to_string()),
                "admin"
            )
            .await
            .unwrap()
    );
}

#[db_test]
async fn is_owner_false(test_db: &kellnr_db::Database) {
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    assert!(
        !test_db
            .is_owner(
                &NormalizedName::from_unchecked("mycrate".to_string()),
                "user"
            )
            .await
            .unwrap()
    );
}

#[db_test]
async fn delete_owner_valid_owner(test_db: &kellnr_db::Database) {
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    test_db.delete_owner("mycrate", "admin").await.unwrap();

    assert!(
        test_db
            .get_crate_owners(&NormalizedName::from_unchecked("mycrate".to_string()))
            .await
            .is_ok()
    );
}

#[db_test]
async fn test_add_crate_duplicate(test_db: &kellnr_db::Database) {
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.1.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    let owners = test_db
        .get_crate_owners(&NormalizedName::from_unchecked("mycrate".to_string()))
        .await
        .unwrap();
    assert_eq!(1, owners.len());
    assert_eq!("admin", owners[0].name);
}

#[db_test]
async fn test_add_crate_different_user(test_db: &kellnr_db::Database) {
    test_db.add_user("user", "123", false, false).await.unwrap();
    let pm = PublishMetadata::minimal("mycrate", "1.0.0");
    let created = Utc::now();

    test_db
        .add_crate(&pm, "cksum", 1024, &created, "admin", None)
        .await
        .unwrap();
    assert!(
        test_db
            .add_crate(&pm, "cksum", 1024, &created, "user", None)
            .await
            .is_err()
    );
}

#[db_test]
async fn test_add_empty_crate(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    test_db.add_empty_crate("mycrate", &created).await.unwrap();

    let normalized_name = NormalizedName::from(OriginalName::try_from("mycrate").unwrap());
    let crate_id = test_db.get_crate_id(&normalized_name).await.unwrap();
    assert!(crate_id.is_some());
}

#[db_test]
async fn test_add_empty_crate_exists(test_db: &kellnr_db::Database) {
    let created = Utc::now();

    // add a crate with version first
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    // now try to add an empty crate with the same name
    let result = test_db.add_empty_crate("mycrate", &created).await;

    assert!(result.is_err());
}

#[db_test]
async fn get_user_from_token_works(test_db: &kellnr_db::Database) {
    test_db
        .add_auth_token("test1", "mytoken1", "admin")
        .await
        .unwrap();

    let user = test_db.get_user_from_token("mytoken1").await.unwrap();

    assert_eq!("admin", user.name);
}

#[db_test]
async fn auth_token_stores_creation_and_last_use(test_db: &kellnr_db::Database) {
    test_db
        .add_auth_token("dated", "dated-token", "admin")
        .await
        .unwrap();
    let token =
        |tokens: Vec<kellnr_db::AuthToken>| tokens.into_iter().find(|t| t.name == "dated").unwrap();

    let created = token(test_db.get_auth_tokens("admin").await.unwrap());
    assert!(created.created.is_some());
    assert_eq!(None, created.last_used);

    test_db.get_user_from_token("dated-token").await.unwrap();
    let used = token(test_db.get_auth_tokens("admin").await.unwrap());
    assert!(used.last_used.is_some());

    // A recent use is not written again, an old one is
    set_auth_token_last_used(test_db, "dated", "2999-01-01 00:00:00")
        .await
        .unwrap();
    test_db.get_user_from_token("dated-token").await.unwrap();
    let recent = token(test_db.get_auth_tokens("admin").await.unwrap());
    assert_eq!(Some("2999-01-01 00:00:00".to_string()), recent.last_used);

    set_auth_token_last_used(test_db, "dated", "2000-01-01 00:00:00")
        .await
        .unwrap();
    test_db.get_user_from_token("dated-token").await.unwrap();
    let old = token(test_db.get_auth_tokens("admin").await.unwrap());
    assert_ne!(Some("2000-01-01 00:00:00".to_string()), old.last_used);
}

#[db_test]
async fn get_auth_tokens_returns_all_tokens(test_db: &kellnr_db::Database) {
    test_db
        .add_auth_token("test1", "mytoken1", "admin")
        .await
        .unwrap();
    test_db
        .add_auth_token("test2", "mytoken2", "admin")
        .await
        .unwrap();

    let tokens = test_db.get_auth_tokens("admin").await.unwrap();

    assert_eq!(3, tokens.len());
    assert_eq!("admin", tokens[0].name);
    assert_eq!("test1", tokens[1].name);
    assert_eq!("test2", tokens[2].name);
}

#[db_test]
async fn auth_token_insert_and_read(test_db: &kellnr_db::Database) {
    test_db
        .add_auth_token("test", "mytoken", "admin")
        .await
        .unwrap();
    let user = test_db.get_user_from_token("mytoken").await.unwrap();

    assert_eq!("admin", user.name);
}

#[db_test]
async fn auth_token_insert_and_delete(test_db: &kellnr_db::Database) {
    test_db
        .add_auth_token("test", "mytoken", "admin")
        .await
        .unwrap();

    test_db.delete_auth_token("admin", 2).await.unwrap();

    assert!(test_db.get_user_from_token("mytoken").await.is_err());
}

#[db_test]
async fn delete_auth_token_of_other_user_fails(test_db: &kellnr_db::Database) {
    test_db
        .add_user("other", "pwd", false, false)
        .await
        .unwrap();
    test_db
        .add_auth_token("test", "mytoken", "admin")
        .await
        .unwrap();

    let result = test_db.delete_auth_token("other", 2).await;

    assert!(matches!(
        result,
        Err(kellnr_db::error::DbError::TokenNotFound)
    ));
    assert_eq!(
        "admin",
        test_db.get_user_from_token("mytoken").await.unwrap().name
    );
    assert!(matches!(
        test_db.delete_auth_token("admin", 99).await,
        Err(kellnr_db::error::DbError::TokenNotFound)
    ));
}

#[db_test]
async fn get_user_from_token_no_token(test_db: &kellnr_db::Database) {
    test_db
        .add_auth_token("test", "mytoken", "admin")
        .await
        .unwrap();

    assert!(test_db.get_user_from_token("wrong_token").await.is_err());
}

#[db_test]
async fn add_auth_token_no_user(test_db: &kellnr_db::Database) {
    assert!(
        test_db
            .add_auth_token("test", "mytoken", "nouser")
            .await
            .is_err()
    );
}

#[db_test]
async fn delete_user_with_sessions(test_db: &kellnr_db::Database) {
    test_db.add_user("user", "pwd", false, false).await.unwrap();
    test_db.add_session_token("user", "123").await.unwrap();
    test_db.add_session_token("user", "abc").await.unwrap();

    test_db.delete_user("user").await.unwrap();

    assert!(test_db.validate_session("123").await.is_err());
    assert!(test_db.validate_session("abc").await.is_err());
    assert!(test_db.get_user("user").await.is_err());
}

#[db_test]
async fn add_user_works(test_db: &kellnr_db::Database) {
    test_db.add_user("user", "pwd", false, false).await.unwrap();

    let user = test_db.get_user("user").await.unwrap();
    // Compare fields individually since created is dynamic
    assert_eq!(2, user.id);
    assert_eq!("user", user.name);
    assert!(user.pwd.starts_with("$argon2id$"), "{}", user.pwd);
    assert!(user.salt.is_empty());
    assert!(!user.is_admin);
    assert!(!user.is_read_only);
    assert!(!user.created.is_empty());
}

#[db_test]
async fn add_user_duplicate(test_db: &kellnr_db::Database) {
    test_db.add_user("user", "pwd", false, false).await.unwrap();

    assert!(test_db.add_user("user", "pwd", false, false).await.is_err());
}

#[db_test]
async fn get_users_works(test_db: &kellnr_db::Database) {
    test_db.add_user("user", "123", false, false).await.unwrap();

    let users = test_db.get_users().await.unwrap();

    assert_eq!(2, users.len());
    assert_eq!("admin", users[0].name);
    assert_eq!("user", users[1].name);
}

#[db_test]
async fn get_user_existing_user(test_db: &kellnr_db::Database) {
    let users = test_db.get_user("admin").await.unwrap();

    assert_eq!("admin", users.name);
}

#[db_test]
async fn get_user_no_user(test_db: &kellnr_db::Database) {
    assert!(test_db.get_user("no_user").await.is_err());
}

#[db_test]
async fn change_pwd_works(test_db: &kellnr_db::Database) {
    test_db.change_pwd("admin", "abc").await.unwrap();

    assert!(test_db.authenticate_user("admin", "abc").await.is_ok());
    assert!(test_db.authenticate_user("admin", "123").await.is_err());
    let admin = test_db.get_user("admin").await.unwrap();
    assert!(admin.pwd.starts_with("$argon2id$"), "{}", admin.pwd);
}

#[db_test]
async fn authenticate_user_upgrades_legacy_hash(test_db: &kellnr_db::Database) {
    set_legacy_pwd(test_db, "admin", "legacy", "salt")
        .await
        .unwrap();

    assert!(test_db.authenticate_user("admin", "legacy").await.is_ok());

    let admin = test_db.get_user("admin").await.unwrap();
    assert!(admin.pwd.starts_with("$argon2id$"), "{}", admin.pwd);
    assert!(admin.salt.is_empty());
    // The upgraded hash keeps accepting the same password.
    assert!(test_db.authenticate_user("admin", "legacy").await.is_ok());
    assert!(test_db.authenticate_user("admin", "wrong").await.is_err());
}

#[db_test]
async fn authenticate_user_keeps_legacy_hash_on_wrong_pwd(test_db: &kellnr_db::Database) {
    set_legacy_pwd(test_db, "admin", "legacy", "salt")
        .await
        .unwrap();

    assert!(test_db.authenticate_user("admin", "wrong").await.is_err());

    let admin = test_db.get_user("admin").await.unwrap();
    assert!(!admin.pwd.starts_with('$'));
    assert_eq!("salt", admin.salt);
}

#[db_test]
async fn is_crate_group_user_works(test_db: &kellnr_db::Database) {
    let group = "group";
    let user = "user";
    test_db.add_user(user, "pwd", false, false).await.unwrap();
    test_db.add_group(group).await.unwrap();
    test_db.add_group_user(group, user).await.unwrap();
    let id = test_add_crate(
        test_db,
        "foobar",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();
    let crate_name_norm = NormalizedName::from_unchecked_str("foobar");
    test_db
        .add_crate_group(&crate_name_norm, group)
        .await
        .unwrap();

    let res = test_db.is_crate_group_user(&crate_name_norm, user).await;
    assert!(res.is_ok_and(|user_is_in_crate_group| user_is_in_crate_group));
}

#[db_test]
async fn clean_db_after_time(test_db: &kellnr_db::Database) {
    test_db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let session = test_db.validate_session("session_token").await.unwrap();
    assert_eq!("admin", session.name);

    let duration = std::time::Duration::from_secs(2);
    std::thread::sleep(duration);
    clean_db(test_db, std::time::Duration::from_secs(1))
        .await
        .unwrap();

    assert!(test_db.validate_session("session_token").await.is_err());
}

/// A leaked database must not hand out usable session cookies, so only a
/// hash of the token is stored.
#[db_test]
async fn session_token_is_stored_hashed(test_db: &kellnr_db::Database) {
    test_db
        .add_session_token("admin", "plain_session_token")
        .await
        .unwrap();

    let stored = stored_session_tokens(test_db).await.unwrap();
    assert_eq!(
        stored,
        [kellnr_db::password::hash_token("plain_session_token")]
    );

    // The stored hash itself is not a valid session token.
    assert!(test_db.validate_session(&stored[0]).await.is_err());
    assert!(
        test_db
            .validate_session("plain_session_token")
            .await
            .is_ok()
    );
}

#[db_test]
async fn delete_user_sessions_removes_all_sessions_of_user(test_db: &kellnr_db::Database) {
    test_db.add_user("user", "pwd", false, false).await.unwrap();
    test_db.add_session_token("admin", "admin_1").await.unwrap();
    test_db.add_session_token("admin", "admin_2").await.unwrap();
    test_db.add_session_token("user", "user_1").await.unwrap();

    let removed = test_db.delete_user_sessions("admin").await.unwrap();

    assert_eq!(removed, 2);
    assert!(test_db.validate_session("admin_1").await.is_err());
    assert!(test_db.validate_session("admin_2").await.is_err());
    assert!(test_db.validate_session("user_1").await.is_ok());
}

#[db_test]
async fn delete_user_sessions_keeps_given_session(test_db: &kellnr_db::Database) {
    test_db.add_session_token("admin", "current").await.unwrap();
    test_db.add_session_token("admin", "other").await.unwrap();

    let removed = test_db
        .delete_other_user_sessions("admin", "current")
        .await
        .unwrap();

    assert_eq!(removed, 1);
    assert!(test_db.validate_session("current").await.is_ok());
    assert!(test_db.validate_session("other").await.is_err());
}

#[db_test]
async fn delete_session_token_works(test_db: &kellnr_db::Database) {
    test_db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let session = test_db.validate_session("session_token").await.unwrap();
    assert_eq!("admin", session.name);

    test_db.delete_session_token("session_token").await.unwrap();

    let r = test_db.validate_session("session_token");
    assert!(r.await.is_err());
}

#[db_test]
async fn delete_session_token_no_token(test_db: &kellnr_db::Database) {
    test_db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let session = test_db.validate_session("session_token").await.unwrap();
    assert_eq!("admin", session.name);

    let r = test_db.delete_session_token("no_token").await;

    assert!(r.is_ok());
}

#[db_test]
async fn get_name_valid_user_and_token(test_db: &kellnr_db::Database) {
    test_db
        .add_session_token("admin", "session_token")
        .await
        .unwrap();
    let session = test_db.validate_session("session_token").await.unwrap();

    assert_eq!("admin", session.name);
}

#[db_test]
async fn oauth2_state_can_be_consumed_once(test_db: &kellnr_db::Database) {
    test_db
        .store_oauth2_state("state", "verifier", "nonce")
        .await
        .unwrap();

    let data = test_db
        .get_and_delete_oauth2_state("state")
        .await
        .unwrap()
        .expect("fresh state must be found");
    assert_eq!(data.pkce_verifier, "verifier");
    assert_eq!(data.nonce, "nonce");

    assert!(
        test_db
            .get_and_delete_oauth2_state("state")
            .await
            .unwrap()
            .is_none()
    );
}

#[db_test]
async fn oauth2_state_expired_is_rejected_and_removed(test_db: &kellnr_db::Database) {
    add_aged_oauth2_state(test_db, "stale", std::time::Duration::from_mins(11))
        .await
        .unwrap();

    assert!(
        test_db
            .get_and_delete_oauth2_state("stale")
            .await
            .unwrap()
            .is_none()
    );
    // The expired row is gone, so cleanup has nothing left to do.
    assert_eq!(test_db.cleanup_expired_oauth2_states().await.unwrap(), 0);
}

#[db_test]
async fn cleanup_expired_oauth2_states_removes_only_old(test_db: &kellnr_db::Database) {
    add_aged_oauth2_state(test_db, "fresh", std::time::Duration::from_mins(1))
        .await
        .unwrap();
    add_aged_oauth2_state(test_db, "stale", std::time::Duration::from_mins(11))
        .await
        .unwrap();

    assert_eq!(test_db.cleanup_expired_oauth2_states().await.unwrap(), 1);
    assert!(
        test_db
            .get_and_delete_oauth2_state("fresh")
            .await
            .unwrap()
            .is_some()
    );
}

#[db_test]
async fn get_session_no_session_in_db(test_db: &kellnr_db::Database) {
    assert!(test_db.validate_session("no_session_token").await.is_err());
}

// Test harness configures a 3600s session age (see db-testcontainer).
#[db_test]
async fn validate_session_rejects_expired(test_db: &kellnr_db::Database) {
    add_aged_session(test_db, "admin", "fresh", std::time::Duration::from_mins(1))
        .await
        .unwrap();
    add_aged_session(
        test_db,
        "admin",
        "stale",
        std::time::Duration::from_hours(2),
    )
    .await
    .unwrap();

    // A session within the age window validates.
    assert!(test_db.validate_session("fresh").await.is_ok());
    // An expired session is rejected (treated as not found), even though the
    // row still exists in the DB.
    assert!(test_db.validate_session("stale").await.is_err());
}

#[db_test]
async fn delete_expired_sessions_removes_only_old(test_db: &kellnr_db::Database) {
    add_aged_session(test_db, "admin", "fresh", std::time::Duration::from_mins(1))
        .await
        .unwrap();
    add_aged_session(
        test_db,
        "admin",
        "stale",
        std::time::Duration::from_hours(2),
    )
    .await
    .unwrap();

    let removed = test_db.delete_expired_sessions().await.unwrap();
    assert_eq!(removed, 1);

    // The fresh session survived; the expired one was removed.
    assert!(test_db.validate_session("fresh").await.is_ok());
    assert!(test_db.validate_session("stale").await.is_err());
}

#[db_test]
async fn validate_session_returns_read_only_and_admin_flags(test_db: &kellnr_db::Database) {
    // read-only, non-admin user
    test_db
        .add_user("rouser", "pwd", false, true)
        .await
        .unwrap();
    test_db
        .add_session_token("rouser", "ro_session")
        .await
        .unwrap();

    let session = test_db.validate_session("ro_session").await.unwrap();
    assert_eq!("rouser", session.name);
    assert!(!session.is_admin);
    assert!(session.is_read_only);

    // default admin user is an admin and not read-only
    test_db
        .add_session_token("admin", "admin_session")
        .await
        .unwrap();
    let admin = test_db.validate_session("admin_session").await.unwrap();
    assert!(admin.is_admin);
    assert!(!admin.is_read_only);
}

#[db_test]
async fn bootstrap_db_inserts_admin(test_db: &kellnr_db::Database) {
    let admin = test_db.get_user("admin").await.unwrap();
    assert_eq!(1, admin.id);
    assert_eq!("admin", admin.name);
    assert!(admin.pwd.starts_with("$argon2id$"), "{}", admin.pwd);
    assert!(admin.salt.is_empty());
    assert!(test_db.authenticate_user("admin", "123").await.is_ok());
    assert!(admin.is_admin);
}

#[db_test]
async fn authenticate_user_valid(test_db: &kellnr_db::Database) {
    assert!(test_db.authenticate_user("admin", "123").await.is_ok());
}

#[db_test]
async fn authenticate_user_unknown_user(test_db: &kellnr_db::Database) {
    assert!(test_db.authenticate_user("unknown", "123").await.is_err());
}

#[db_test]
async fn authenticate_user_wrong_pwd(test_db: &kellnr_db::Database) {
    assert!(test_db.authenticate_user("admin", "abc").await.is_err());
}

#[db_test]
async fn crate_version_exists_with_existing_version(test_db: &kellnr_db::Database) {
    let id = test_add_crate(
        test_db,
        "foobar",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();
    assert!(
        test_add_crate_meta(test_db, id, "1.0.0", &Utc::now(), None)
            .await
            .is_err()
    );
}

#[db_test]
async fn crate_version_exists_with_no_existing_version(test_db: &kellnr_db::Database) {
    let id = test_add_crate(
        test_db,
        "foobar",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    assert!(!test_db.crate_version_exists(id, "2.0.0").await.unwrap());
}

#[db_test]
async fn get_total_unique_crates_returns_correct_number(test_db: &kellnr_db::Database) {
    let _ = test_add_crate(
        test_db,
        "foobar",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    let _ = test_add_crate(
        test_db,
        "bar",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    assert_eq!(2, test_db.get_total_unique_crates().await.unwrap());
}

#[db_test]
async fn add_and_get_doc_queue_entries(test_db: &kellnr_db::Database) {
    test_db
        .add_doc_queue(
            &NormalizedName::from_unchecked("my_crate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            &PathBuf::from("/tmp/foo"),
        )
        .await
        .unwrap();
    test_db
        .add_doc_queue(
            &NormalizedName::from_unchecked("my_crate2".to_string()),
            &Version::try_from("2.0.0").unwrap(),
            &PathBuf::from("/tmp/bar"),
        )
        .await
        .unwrap();

    let queue_entries = test_db.get_doc_queue().await.unwrap();

    assert_eq!(
        DocQueueEntry {
            id: 1,
            normalized_name: NormalizedName::from_unchecked("my_crate".to_string()),
            version: "1.0.0".to_string(),
            path: PathBuf::from("/tmp/foo")
        },
        queue_entries[0]
    );

    assert_eq!(
        DocQueueEntry {
            id: 2,
            normalized_name: NormalizedName::from_unchecked("my_crate2".to_string()),
            version: "2.0.0".to_string(),
            path: PathBuf::from("/tmp/bar")
        },
        queue_entries[1]
    );
}

#[db_test]
async fn add_doc_queue_rejects_non_utf8_path(test_db: &kellnr_db::Database) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;

        let path = PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/inv\xffalid"));
        let result = test_db
            .add_doc_queue(
                &NormalizedName::from_unchecked("my_crate".to_string()),
                &Version::try_from("1.0.0").unwrap(),
                &path,
            )
            .await;

        assert!(matches!(
            result,
            Err(kellnr_db::error::DbError::NonUtf8Path(_))
        ));
        assert!(test_db.get_doc_queue().await.unwrap().is_empty());
    }
}

#[db_test]
async fn delete_doc_queue_entry(test_db: &kellnr_db::Database) {
    test_db
        .add_doc_queue(
            &NormalizedName::from_unchecked("my_crate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            &PathBuf::from("/tmp/foo"),
        )
        .await
        .unwrap();
    test_db
        .add_doc_queue(
            &NormalizedName::from_unchecked("my_crate2".to_string()),
            &Version::try_from("2.0.0").unwrap(),
            &PathBuf::from("/tmp/bar"),
        )
        .await
        .unwrap();

    let queue_entries = test_db.get_doc_queue().await.unwrap();
    assert_eq!(2, queue_entries.len());
    test_db.delete_doc_queue(1).await.unwrap();

    let queue_entries = test_db.get_doc_queue().await.unwrap();
    assert_eq!(1, queue_entries.len());
    assert_eq!(
        DocQueueEntry {
            id: 2,
            normalized_name: NormalizedName::from_unchecked("my_crate2".to_string()),
            version: "2.0.0".to_string(),
            path: PathBuf::from("/tmp/bar")
        },
        queue_entries[0]
    );
}

#[db_test]
async fn delete_crate_one_of_multiple_versions(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("3.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    let total_versions_before = test_db.get_total_crate_versions().await.unwrap();

    test_db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::try_from("2.0.0").unwrap(),
        )
        .await
        .unwrap();

    let summaries = get_crate_meta_list(test_db, 1).await.unwrap();
    let krate = test_db
        .get_crate_id(&NormalizedName::from_unchecked("crate".to_string()))
        .await
        .unwrap();
    assert_eq!(3, total_versions_before);
    assert_eq!(2, summaries.len());
    assert_eq!("1.0.0", summaries[0].version);
    assert_eq!("3.0.0", summaries[1].version);
    assert_eq!(1, krate.unwrap());
}

#[db_test]
async fn delete_crate_max_version(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("3.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    let total_versions_before = test_db.get_total_crate_versions().await.unwrap();

    test_db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::try_from("3.0.0").unwrap(),
        )
        .await
        .unwrap();

    let crate_metas = get_crate_meta_list(test_db, 1).await.unwrap();
    let krate = test_db
        .get_crate_id(&NormalizedName::from_unchecked("crate".to_string()))
        .await
        .unwrap()
        .unwrap();
    let max_version = test_db.get_max_version_from_id(krate).await.unwrap();
    assert_eq!(3, total_versions_before);
    assert_eq!(2, crate_metas.len());
    assert_eq!("1.0.0", crate_metas[0].version);
    assert_eq!("2.0.0", crate_metas[1].version);
    assert_eq!("2.0.0", max_version.to_string());
    assert_eq!(1, krate);
}

#[db_test]
async fn delete_crate_only_versions(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    let total_versions_before = test_db.get_total_crate_versions().await.unwrap();

    test_db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::try_from("1.0.0").unwrap(),
        )
        .await
        .unwrap();

    let summaries = get_crate_meta_list(test_db, 1).await.unwrap();
    let krate = test_db
        .get_crate_id(&NormalizedName::from_unchecked("crate".to_string()))
        .await
        .unwrap();
    assert_eq!(1, total_versions_before);
    assert_eq!(0, summaries.len());
    assert!(krate.is_none());
}

async fn search_and_count_test_fixtures(test_db: &kellnr_db::Database) -> Vec<CrateOverview> {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created_string = created.format("%Y-%m-%d %H:%M:%S").to_string();
    test_add_crate_with_downloads(
        test_db,
        "crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        None,
        &created,
        Some(4),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "crate",
        "admin",
        &Version::try_from("2.2.0").unwrap(),
        None,
        &created,
        Some(4),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "foo_crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        None,
        &created,
        Some(3),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "foo_crate",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        Some("this is the foo crate"),
        &created,
        Some(3),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "crate_foo",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        None,
        &created,
        Some(5),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "crate_foo",
        "admin",
        &Version::try_from("3.0.0").unwrap(),
        None,
        &created,
        Some(5),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "only_in_description",
        "admin",
        &Version::try_from("4.0.0").unwrap(),
        Some("description contains the word 'Crate' in it"),
        &created,
        Some(1),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "no_match",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        None,
        &created,
        Some(1),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "no_match",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        None,
        &created,
        Some(1),
    )
    .await
    .unwrap();

    // Expected results when searching for "crate":
    vec![
        CrateOverview {
            name: "crate".to_string(),
            version: "2.2.0".to_string(),
            date: created_string.clone(),
            total_downloads: 8,
            ..CrateOverview::default()
        },
        CrateOverview {
            name: "crate_foo".to_string(),
            version: "3.0.0".to_string(),
            date: created_string.clone(),
            total_downloads: 10,
            ..CrateOverview::default()
        },
        CrateOverview {
            name: "foo_crate".to_string(),
            version: "2.0.0".to_string(),
            description: Some("this is the foo crate".to_string()),
            date: created_string.clone(),
            total_downloads: 6,
            ..CrateOverview::default()
        },
        CrateOverview {
            name: "only_in_description".to_string(),
            version: "4.0.0".to_string(),
            description: Some("description contains the word 'Crate' in it".to_string()),
            date: created_string.clone(),
            total_downloads: 1,
            ..CrateOverview::default()
        },
    ]
}

#[db_test]
async fn search_in_crate_name_and_description_found_match(test_db: &kellnr_db::Database) {
    let expected = search_and_count_test_fixtures(test_db).await;

    let results_1 = test_db
        .search_in_crate_name_and_description("crate", 2, 0, false, &CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(expected[0..2].to_vec(), results_1);

    let results_2 = test_db
        .search_in_crate_name_and_description("crate", 2, 2, false, &CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(expected[2..4].to_vec(), results_2);

    let search_results = test_db
        .search_in_crate_name_and_description("crate", 100, 0, false, &CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(expected, search_results);
}

#[db_test]
async fn search_in_crate_name_and_description_ranks_name_matches_first(
    test_db: &kellnr_db::Database,
) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let version = Version::try_from("1.0.0").unwrap();
    for (name, description) in [
        ("a_serde_helper", None),
        ("serde_json", None),
        ("zz_desc", Some("works with Serde")),
    ] {
        test_add_crate_with_downloads(
            test_db,
            name,
            "admin",
            &version,
            description,
            &created,
            None,
        )
        .await
        .unwrap();
    }
    // The exact match is a cached crates.io crate, so the ranking has to
    // work across both parts of the union
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked("serde".to_string()),
            "etag",
            "last_modified",
            None,
            &[IndexMetadata {
                name: "serde".to_string(),
                vers: "1.0.0".to_string(),
                deps: vec![],
                cksum: "cksum".to_string(),
                features: BTreeMap::default(),
                yanked: false,
                links: None,
                pubtime: None,
                v: Some(1),
                features2: None,
                rust_version: None,
            }],
            &HashMap::new(),
        )
        .await
        .unwrap();

    let names =
        |results: Vec<CrateOverview>| results.into_iter().map(|c| c.name).collect::<Vec<_>>();

    // Exact match, then prefix, then substring, then description only;
    // alphabetical order would put a_serde_helper first
    let results = test_db
        .search_in_crate_name_and_description("SerDe", 100, 0, true, &CrateVisibility::All)
        .await
        .unwrap();
    assert_eq!(
        names(results),
        ["serde", "serde_json", "a_serde_helper", "zz_desc"]
    );

    // Paging follows the same order
    let results = test_db
        .search_in_crate_name_and_description("serde", 2, 2, true, &CrateVisibility::All)
        .await
        .unwrap();
    assert_eq!(names(results), ["a_serde_helper", "zz_desc"]);

    // The crate overview without a query stays alphabetical
    let results = test_db
        .get_crate_overview_list(
            100,
            0,
            true,
            &CrateVisibility::All,
            CrateSort::Name,
            &CrateTagFilter::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        names(results),
        ["a_serde_helper", "serde", "serde_json", "zz_desc"]
    );
}

// Every match has to stay reachable by paging further, no matter how many there
// are. The web UI search used to ask for a single fixed window of 100 rows, so
// anything past that was silently dropped and could not be requested again.
#[db_test]
async fn search_in_crate_name_and_description_reaches_results_past_the_first_pages(
    test_db: &kellnr_db::Database,
) {
    const TOTAL: usize = 150;
    const PAGE_SIZE: u64 = 20;

    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    for i in 0..TOTAL {
        test_add_crate(
            test_db,
            &format!("crate{i:03}"),
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
    }

    let mut paged = Vec::new();
    let mut page = 0;
    loop {
        let hits = test_db
            .search_in_crate_name_and_description(
                "crate",
                PAGE_SIZE,
                PAGE_SIZE * page,
                false,
                &CrateVisibility::All,
            )
            .await
            .unwrap();
        let hit_count = hits.len() as u64;
        paged.extend(hits.into_iter().map(|c| c.name));
        if hit_count < PAGE_SIZE {
            break;
        }
        page += 1;
    }

    let expected = (0..TOTAL)
        .map(|i| format!("crate{i:03}"))
        .collect::<Vec<_>>();

    assert_eq!(expected, paged);
}

#[db_test]
async fn count_by_crate_name_and_description_found_match(test_db: &kellnr_db::Database) {
    let expected = search_and_count_test_fixtures(test_db).await;

    let results = test_db
        .count_by_crate_name_and_description("crate", false, &CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(expected.len(), results as usize);

    let results = test_db
        .count_by_crate_name_and_description("something not found", false, &CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(0, results);
}

#[db_test]
async fn count_by_crate_name_and_description_with_cache(test_db: &kellnr_db::Database) {
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked("crate".to_string()),
            "etag",
            "last_modified",
            None,
            &[IndexMetadata {
                name: "crate".to_string(),
                vers: "9.0.0".to_string(),
                deps: vec![],
                cksum: "cksum".to_string(),
                features: BTreeMap::default(),
                yanked: false,
                links: None,
                pubtime: None,
                v: Some(1),
                features2: None,
                rust_version: None,
            }],
            &HashMap::new(),
        )
        .await
        .unwrap();

    let expected = search_and_count_test_fixtures(test_db).await;

    let results = test_db
        .count_by_crate_name_and_description("crate", true, &CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(expected.len() + 1, results as usize);

    let results = test_db
        .count_by_crate_name_and_description("something not found", true, &CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(0, results);
}

#[db_test]
async fn get_crate_overview_list(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created_string = created.format("%Y-%m-%d %H:%M:%S").to_string();
    test_add_crate_with_downloads(
        test_db,
        "crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        None,
        &created,
        Some(4),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "crate",
        "admin",
        &Version::try_from("2.2.0").unwrap(),
        None,
        &created,
        Some(4),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "foo_crate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        None,
        &created,
        Some(3),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "foo_crate",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        None,
        &created,
        Some(3),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "crate_foo",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        None,
        &created,
        Some(5),
    )
    .await
    .unwrap();
    test_add_crate_with_downloads(
        test_db,
        "crate_foo",
        "admin",
        &Version::try_from("3.0.0").unwrap(),
        None,
        &created,
        Some(5),
    )
    .await
    .unwrap();
    let expected = vec![
        CrateOverview {
            name: "crate".to_string(),
            version: "2.2.0".to_string(),
            date: created_string.clone(),
            total_downloads: 8,
            ..CrateOverview::default()
        },
        CrateOverview {
            name: "crate_foo".to_string(),
            version: "3.0.0".to_string(),
            date: created_string.clone(),
            total_downloads: 10,
            ..CrateOverview::default()
        },
        CrateOverview {
            name: "foo_crate".to_string(),
            version: "2.0.0".to_string(),
            date: created_string.clone(),
            total_downloads: 6,
            ..CrateOverview::default()
        },
    ];

    let overview_list = test_db
        .get_crate_overview_list(
            10,
            0,
            false,
            &CrateVisibility::All,
            CrateSort::Name,
            &CrateTagFilter::default(),
        )
        .await
        .unwrap();

    assert_eq!(expected, overview_list);

    let total = test_db
        .count_crate_overview_list(false, &CrateVisibility::All, &CrateTagFilter::default())
        .await
        .unwrap();
    assert_eq!(3, total);
}

#[db_test]
async fn get_crate_overview_list_sorted_by_date_and_downloads(test_db: &kellnr_db::Database) {
    let day = |d| Utc.with_ymd_and_hms(2020, 10, d, 13, 18, 00).unwrap();
    for (name, created, downloads) in [
        ("alpha", day(3), 7),
        ("beta", day(9), 1),
        ("gamma", day(5), 7),
    ] {
        test_add_crate_with_downloads(
            test_db,
            name,
            "admin",
            &Version::try_from("1.0.0").unwrap(),
            None,
            &created,
            Some(downloads),
        )
        .await
        .unwrap();
    }
    let names = |list: Vec<CrateOverview>| list.into_iter().map(|c| c.name).collect::<Vec<_>>();

    let by_date = test_db
        .get_crate_overview_list(
            10,
            0,
            false,
            &CrateVisibility::All,
            CrateSort::Updated,
            &CrateTagFilter::default(),
        )
        .await
        .unwrap();
    assert_eq!(vec!["beta", "gamma", "alpha"], names(by_date));

    // Equal download counts are ordered by name
    let by_downloads = test_db
        .get_crate_overview_list(
            10,
            0,
            false,
            &CrateVisibility::All,
            CrateSort::Downloads,
            &CrateTagFilter::default(),
        )
        .await
        .unwrap();
    assert_eq!(vec!["alpha", "gamma", "beta"], names(by_downloads));

    // Paging keeps the order
    let second_page = test_db
        .get_crate_overview_list(
            2,
            2,
            false,
            &CrateVisibility::All,
            CrateSort::Downloads,
            &CrateTagFilter::default(),
        )
        .await
        .unwrap();
    assert_eq!(vec!["beta"], names(second_page));
}

#[db_test]
async fn get_crate_overview_list_filters_by_keyword_and_category(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    for (name, keywords, categories) in [
        (
            "alpha",
            vec!["cli", "Parser"],
            vec!["command-line-utilities"],
        ),
        ("beta", vec!["cli"], vec![]),
        ("gamma", vec![], vec!["command-line-utilities"]),
    ] {
        let pm = PublishMetadata {
            keywords: keywords.into_iter().map(String::from).collect(),
            categories: categories.into_iter().map(String::from).collect(),
            repository: Some(format!("https://example.com/{name}")),
            ..PublishMetadata::minimal(name, "1.0.0")
        };
        test_db
            .add_crate(&pm, "cksum", 1024, &created, "admin", None)
            .await
            .unwrap();
    }
    let list = |keyword: Option<&str>, category: Option<&str>| {
        let tags = CrateTagFilter {
            keyword: keyword.map(String::from),
            category: category.map(String::from),
        };
        async move {
            let crates = test_db
                .get_crate_overview_list(10, 0, true, &CrateVisibility::All, CrateSort::Name, &tags)
                .await
                .unwrap();
            let total = test_db
                .count_crate_overview_list(true, &CrateVisibility::All, &tags)
                .await
                .unwrap();
            (crates, total)
        }
    };
    let names = |list: &[CrateOverview]| list.iter().map(|c| c.name.clone()).collect::<Vec<_>>();

    let (crates, total) = list(Some("cli"), None).await;
    assert_eq!(vec!["alpha", "beta"], names(&crates));
    assert_eq!(2, total);
    assert_eq!(
        Some("https://example.com/alpha".to_string()),
        crates[0].repository
    );

    // Keywords match regardless of case
    let (crates, total) = list(Some("parser"), None).await;
    assert_eq!(vec!["alpha"], names(&crates));
    assert_eq!(1, total);

    let (crates, total) = list(None, Some("command-line-utilities")).await;
    assert_eq!(vec!["alpha", "gamma"], names(&crates));
    assert_eq!(2, total);

    // Both filters have to match
    let (crates, total) = list(Some("cli"), Some("command-line-utilities")).await;
    assert_eq!(vec!["alpha"], names(&crates));
    assert_eq!(1, total);

    let (crates, total) = list(Some("unknown"), None).await;
    assert!(crates.is_empty());
    assert_eq!(0, total);
}

#[db_test]
async fn get_crate_data_returns_size_and_rust_version(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    let pm = PublishMetadata {
        rust_version: Some("1.85".to_string()),
        ..PublishMetadata::minimal("sized", "1.0.0")
    };
    test_db
        .add_crate(&pm, "cksum", 4711, &created, "admin", None)
        .await
        .unwrap();

    let data = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("sized"))
        .await
        .unwrap();

    assert_eq!(4711, data.versions[0].size);
    assert_eq!(Some("1.85".to_string()), data.versions[0].rust_version);
}

#[db_test]
async fn get_crate_data_returns_publisher(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    test_db
        .add_crate(
            &PublishMetadata::minimal("published", "1.0.0"),
            "cksum",
            1024,
            &created,
            "admin",
            Some("ci".to_string()),
        )
        .await
        .unwrap();
    test_db
        .add_crate(
            &PublishMetadata::minimal("published", "1.1.0"),
            "cksum",
            1024,
            &created,
            "admin",
            None,
        )
        .await
        .unwrap();

    let data = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("published"))
        .await
        .unwrap();

    let version = |v: &str| data.versions.iter().find(|cv| cv.version == v).unwrap();
    assert_eq!(Some("admin".to_string()), version("1.0.0").publisher);
    assert_eq!(Some("ci".to_string()), version("1.0.0").publisher_token);
    assert_eq!(Some("admin".to_string()), version("1.1.0").publisher);
    assert_eq!(None, version("1.1.0").publisher_token);
}

#[db_test]
async fn get_auth_token_name_finds_the_token(test_db: &kellnr_db::Database) {
    test_db
        .add_auth_token("deploy", "secret-token", "admin")
        .await
        .unwrap();

    assert_eq!(
        Some("deploy".to_string()),
        test_db.get_auth_token_name("secret-token").await.unwrap()
    );
    assert_eq!(
        None,
        test_db.get_auth_token_name("other-token").await.unwrap()
    );
}

#[db_test]
async fn get_crate_data_returns_latest_version_of_dependencies(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    for version in ["1.0.0", "2.0.0", "3.0.0", "4.0.0-beta.1"] {
        let pm = PublishMetadata {
            description: Some("A library".to_string()),
            ..PublishMetadata::minimal("dep-lib", version)
        };
        test_db
            .add_crate(&pm, "cksum", 1024, &created, "admin", None)
            .await
            .unwrap();
    }
    // Yanked versions and pre-releases do not count as the latest version
    test_db
        .yank_crate(
            &NormalizedName::from_unchecked_str("dep-lib"),
            &Version::from_unchecked_str("3.0.0"),
        )
        .await
        .unwrap();

    let dep = |req: &str, rename: Option<&str>| RegistryDep {
        name: "dep-lib".to_string(),
        version_req: req.to_string(),
        features: None,
        optional: false,
        default_features: true,
        target: None,
        kind: Some("normal".to_string()),
        registry: None,
        explicit_name_in_toml: rename.map(str::to_string),
    };
    let pm = PublishMetadata {
        deps: vec![
            dep("^1.0", None),
            dep("^2.0", Some("renamed")),
            RegistryDep {
                name: "unknown".to_string(),
                ..dep("^1.0", None)
            },
        ],
        ..PublishMetadata::minimal("app", "1.0.0")
    };
    test_db
        .add_crate(&pm, "cksum", 1024, &created, "admin", None)
        .await
        .unwrap();

    let data = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("app"))
        .await
        .unwrap();
    let deps = &data.versions[0].dependencies;
    let find = |req: &str, name: &str| {
        deps.iter()
            .find(|d| d.version_req == req && d.name == name)
            .unwrap()
    };

    let old = find("^1.0", "dep-lib");
    assert_eq!(Some("2.0.0".to_string()), old.latest_version);
    assert!(old.outdated);
    assert_eq!(Some("A library".to_string()), old.description);

    // Renamed dependencies are looked up by their package name
    let current = find("^2.0", "dep-lib");
    assert_eq!(Some("renamed".to_string()), current.explicit_name_in_toml);
    assert_eq!(Some("2.0.0".to_string()), current.latest_version);
    assert!(!current.outdated);
    assert_eq!(Some("A library".to_string()), current.description);

    let unknown = find("^1.0", "unknown");
    assert_eq!(None, unknown.latest_version);
    assert!(!unknown.outdated);
}

#[db_test]
async fn add_crate_and_get_crate_data(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created_string = created.format("%Y-%m-%d %H:%M:%S").to_string();
    let pm1_v1 = PublishMetadata {
        name: "crate1".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![
            RegistryDep {
                name: "dep1".to_string(),
                version_req: "5.0.0".to_string(),
                features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
                optional: false,
                default_features: true,
                target: Some("normal".to_string()),
                kind: Some("kind1".to_string()),
                registry: Some("registry1".to_string()),
                explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
            },
            RegistryDep {
                name: "dep2".to_string(),
                version_req: "6.0.0".to_string(),
                features: Some(vec!["feature3".to_string(), "feature4".to_string()]),
                optional: true,
                default_features: false,
                target: Some("normal".to_string()),
                kind: Some("kind2".to_string()),
                registry: Some("registry2".to_string()),
                explicit_name_in_toml: None,
            },
        ],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        documentation: Some("documentation1".to_string()),
        homepage: Some("homepage1".to_string()),
        readme: Some("readme1".to_string()),
        readme_file: Some("readme_file1".to_string()),
        keywords: vec!["keyword1".to_string(), "keyword2".to_string()],
        categories: vec!["category1".to_string(), "category2".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        repository: Some("repository1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
        rust_version: None,
    };
    let pm1_v2 = PublishMetadata {
        name: "crate1".to_string(),
        vers: "2.0.0".to_string(),
        deps: vec![
            RegistryDep {
                name: "dep1".to_string(),
                version_req: "5.0.0".to_string(),
                features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
                optional: false,
                default_features: true,
                target: Some("normal".to_string()),
                kind: Some("kind1".to_string()),
                registry: Some("registry1".to_string()),
                explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
            },
            RegistryDep {
                name: "dep2".to_string(),
                version_req: "6.0.0".to_string(),
                features: Some(vec!["feature3".to_string(), "feature4".to_string()]),
                optional: true,
                default_features: false,
                target: Some("normal".to_string()),
                kind: Some("kind2".to_string()),
                registry: Some("registry2".to_string()),
                explicit_name_in_toml: None,
            },
        ],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec![
            "author1".to_string(),
            "author2".to_string(),
            "author3".to_string(),
        ]),
        description: Some("description2".to_string()),
        documentation: Some("documentation2".to_string()),
        homepage: Some("homepage2".to_string()),
        readme: Some("readme2".to_string()),
        readme_file: Some("readme_file2".to_string()),
        keywords: vec!["keyword1".to_string()],
        categories: vec!["category1".to_string(), "category4".to_string()],
        license: Some("license2".to_string()),
        license_file: Some("license_file2".to_string()),
        repository: Some("repository2".to_string()),
        badges: None,
        links: Some("links2".to_string()),
        rust_version: None,
    };
    let pm2_v1 = PublishMetadata {
        name: "crate2".to_string(),
        vers: "3.0.0".to_string(),
        deps: vec![
            RegistryDep {
                name: "dep1".to_string(),
                version_req: "5.0.0".to_string(),
                features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
                optional: false,
                default_features: true,
                target: Some("normal".to_string()),
                kind: Some("kind1".to_string()),
                registry: Some("registry1".to_string()),
                explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
            },
            RegistryDep {
                name: "dep2".to_string(),
                version_req: "6.0.0".to_string(),
                features: Some(vec!["feature3".to_string(), "feature4".to_string()]),
                optional: true,
                default_features: false,
                target: Some("normal".to_string()),
                kind: Some("kind2".to_string()),
                registry: Some("registry2".to_string()),
                explicit_name_in_toml: None,
            },
        ],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        documentation: Some("documentation1".to_string()),
        homepage: Some("homepage1".to_string()),
        readme: Some("readme1".to_string()),
        readme_file: Some("readme_file1".to_string()),
        keywords: vec!["keyword1".to_string(), "keyword2".to_string()],
        categories: vec!["category1".to_string(), "category2".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        repository: Some("repository1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
        rust_version: None,
    };
    let pm2_v2 = PublishMetadata {
        name: "crate2".to_string(),
        vers: "4.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep1".to_string(),
            version_req: "5.0.0".to_string(),
            features: Some(vec!["feature1".to_string(), "feature2".to_string()]),
            optional: false,
            default_features: true,
            target: Some("normal".to_string()),
            kind: Some("kind1".to_string()),
            registry: Some("registry1".to_string()),
            explicit_name_in_toml: Some("explicit_name_in_toml1".to_string()),
        }],
        ..Default::default()
    };
    test_db
        .add_user("owner1", "pwd1", false, false)
        .await
        .unwrap();
    test_db
        .add_user("owner2", "pwd2", false, false)
        .await
        .unwrap();

    // Test, if adding a new crate with multiple versions works as expected
    test_db
        .add_crate(&pm1_v1, "cksum1_1", 1024, &created, "owner1", None)
        .await
        .unwrap();
    test_db
        .add_owner(
            &NormalizedName::from_unchecked("crate1".to_string()),
            "owner2",
        )
        .await
        .unwrap();

    let crate_data1_v1 = test_db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    let etag1_v1 = test_db.get_prefetch_data("crate1").await.unwrap().etag;

    assert_eq!(
        CrateData {
            name: pm1_v1.name.clone(),
            owners: vec!["owner1".to_string(), "owner2".to_string()],
            max_version: pm1_v1.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm1_v1.homepage.clone(),
            description: pm1_v1.description.clone(),
            repository: pm1_v1.repository.clone(),
            categories: pm1_v1.categories.clone(),
            keywords: pm1_v1.keywords.clone(),
            authors: pm1_v1.authors.clone().unwrap(),
            versions: vec![CrateVersionData {
                version: pm1_v1.vers.clone(),
                created: created_string.clone(),
                downloads: 0,
                documentation: pm1_v1.documentation.clone(),
                readme: pm1_v1.readme.clone(),
                license: pm1_v1.license.clone(),
                license_file: pm1_v1.license_file.clone(),
                dependencies: pm1_v1
                    .deps
                    .clone()
                    .into_iter()
                    .map(CrateRegistryDep::from)
                    .collect(),
                checksum: "cksum1_1".to_string(),
                features: pm1_v1.features.clone(),
                yanked: false,
                links: pm1_v1.links.clone(),
                v: 1,
                size: 1024,
                rust_version: pm1_v1.rust_version.clone(),
                publisher: Some("owner1".to_string()),
                publisher_token: None,
            }],
        },
        crate_data1_v1
    );

    test_db
        .add_crate(&pm1_v2, "cksum1_2", 1024, &created, "owner1", None)
        .await
        .unwrap();

    let crate_data1_v2 = test_db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    let etag1_v2 = test_db.get_prefetch_data("crate1").await.unwrap().etag;

    assert_eq!(
        CrateData {
            name: pm1_v2.name.clone(),
            owners: vec!["owner1".to_string(), "owner2".to_string()],
            max_version: pm1_v2.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm1_v2.homepage.clone(),
            description: pm1_v2.description.clone(),
            repository: pm1_v2.repository.clone(),
            categories: pm1_v2.categories.clone(),
            keywords: pm1_v2.keywords.clone(),
            authors: pm1_v2.authors.clone().unwrap(),
            versions: vec![
                CrateVersionData {
                    version: pm1_v2.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    readme: pm1_v2.readme.clone(),
                    license: pm1_v2.license.clone(),
                    license_file: pm1_v2.license_file.clone(),
                    documentation: pm1_v2.documentation.clone(),
                    dependencies: pm1_v2
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    checksum: "cksum1_2".to_string(),
                    features: pm1_v2.features.clone(),
                    yanked: false,
                    links: pm1_v2.links.clone(),
                    v: 1,
                    size: 1024,
                    rust_version: pm1_v2.rust_version.clone(),
                    publisher: Some("owner1".to_string()),
                    publisher_token: None,
                },
                CrateVersionData {
                    version: pm1_v1.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    readme: pm1_v1.readme.clone(),
                    license: pm1_v1.license.clone(),
                    license_file: pm1_v1.license_file.clone(),
                    documentation: pm1_v1.documentation.clone(),
                    dependencies: pm1_v1
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    checksum: "cksum1_1".to_string(),
                    features: pm1_v1.features.clone(),
                    yanked: false,
                    links: pm1_v1.links.clone(),
                    v: 1,
                    size: 1024,
                    rust_version: pm1_v1.rust_version.clone(),
                    publisher: Some("owner1".to_string()),
                    publisher_token: None,
                }
            ],
        },
        crate_data1_v2
    );

    // Check that the etag changed with newly published version
    assert_ne!(etag1_v1, etag1_v2);

    test_db
        .add_crate(&pm2_v1, "cksum2_1", 1024, &created, "owner2", None)
        .await
        .unwrap();

    let crate_data2_v1 = test_db
        .get_crate_data(&NormalizedName::from_unchecked("crate2".to_string()))
        .await
        .unwrap();

    assert_eq!(
        CrateData {
            name: pm2_v1.name.clone(),
            owners: vec!["owner2".to_string()],
            max_version: pm2_v1.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm2_v1.homepage.clone(),
            description: pm2_v1.description.clone(),
            repository: pm2_v1.repository.clone(),
            categories: pm2_v1.categories.clone(),
            keywords: pm2_v1.keywords.clone(),
            authors: pm2_v1.authors.clone().unwrap(),
            versions: vec![CrateVersionData {
                version: pm2_v1.vers.clone(),
                created: created_string.clone(),
                downloads: 0,
                readme: pm2_v1.readme.clone(),
                license: pm2_v1.license.clone(),
                license_file: pm2_v1.license_file.clone(),
                documentation: pm2_v1.documentation.clone(),
                dependencies: pm2_v1
                    .deps
                    .clone()
                    .into_iter()
                    .map(CrateRegistryDep::from)
                    .collect(),
                checksum: "cksum2_1".to_string(),
                features: pm2_v1.features.clone(),
                yanked: false,
                links: pm2_v1.links.clone(),
                v: 1,
                size: 1024,
                rust_version: pm2_v1.rust_version.clone(),
                publisher: Some("owner2".to_string()),
                publisher_token: None,
            }],
        },
        crate_data2_v1
    );

    test_db
        .add_crate(&pm2_v2, "cksum2_2", 1024, &created, "owner2", None)
        .await
        .unwrap();

    let crate_data2_v2 = test_db
        .get_crate_data(&NormalizedName::from_unchecked("crate2".to_string()))
        .await
        .unwrap();

    assert_eq!(
        CrateData {
            name: pm2_v2.name.clone(),
            owners: vec!["owner2".to_string()],
            max_version: pm2_v2.vers.clone(),
            total_downloads: 0,
            last_updated: created_string.clone(),
            homepage: pm2_v2.homepage.clone(),
            description: pm2_v2.description.clone(),
            repository: pm2_v2.repository.clone(),
            categories: Vec::default(),
            keywords: Vec::default(),
            authors: Vec::default(),
            versions: vec![
                CrateVersionData {
                    version: pm2_v2.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    readme: pm2_v2.readme.clone(),
                    license: pm2_v2.license.clone(),
                    license_file: pm2_v2.license_file.clone(),
                    documentation: pm2_v2.documentation.clone(),
                    dependencies: pm2_v2
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    checksum: "cksum2_2".to_string(),
                    features: BTreeMap::default(),
                    yanked: false,
                    links: pm2_v2.links.clone(),
                    v: 1,
                    size: 1024,
                    rust_version: pm2_v2.rust_version.clone(),
                    publisher: Some("owner2".to_string()),
                    publisher_token: None,
                },
                CrateVersionData {
                    version: pm2_v1.vers.clone(),
                    created: created_string.clone(),
                    downloads: 0,
                    documentation: pm2_v1.documentation.clone(),
                    license: pm2_v1.license.clone(),
                    license_file: pm2_v1.license_file.clone(),
                    dependencies: pm2_v1
                        .deps
                        .into_iter()
                        .map(CrateRegistryDep::from)
                        .collect(),
                    checksum: "cksum2_1".to_string(),
                    features: pm2_v1.features.clone(),
                    yanked: false,
                    links: pm2_v1.links.clone(),
                    v: 1,
                    size: 1024,
                    rust_version: pm2_v1.rust_version.clone(),
                    publisher: Some("owner2".to_string()),
                    publisher_token: None,
                    readme: pm2_v1.readme.clone(),
                }
            ],
        },
        crate_data2_v2
    );
}

#[db_test]
async fn update_docs_link(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let pm = PublishMetadata::minimal("crate1", "1.0.0");
    test_db
        .add_user("owner1", "pwd", false, false)
        .await
        .unwrap();
    test_db
        .add_crate(&pm, "cksum1_1", 1024, &created, "owner1", None)
        .await
        .unwrap();

    let crate_before = test_db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    assert_eq!(crate_before.versions[0].documentation, None);

    test_db
        .update_docs_link(
            &NormalizedName::from_unchecked("crate1".to_string()),
            &Version::try_from("1.0.0").unwrap(),
            "https://docs.rs/crate1/1.0.0/crate1/",
        )
        .await
        .unwrap();

    let crate_after = test_db
        .get_crate_data(&NormalizedName::from_unchecked("crate1".to_string()))
        .await
        .unwrap();
    assert_eq!(
        crate_after.versions[0].documentation,
        Some("https://docs.rs/crate1/1.0.0/crate1/".to_string())
    );
}

#[db_test]
async fn get_prefetch_data_with_minimal_data(test_db: &kellnr_db::Database) {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::from_unchecked_str("1.0.0"),
        &created1,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::from_unchecked_str("2.0.0"),
        &created2,
    )
    .await
    .unwrap();

    let prefetch_data = test_db.get_prefetch_data("crate").await.unwrap();

    // etag changes with pubtime (set to Utc::now()), so just verify it's not empty
    assert!(!prefetch_data.etag.is_empty());
    assert_eq!(
        created2.format("%Y-%m-%d %H:%M:%S").to_string(),
        prefetch_data.last_modified
    );
    // data length varies with pubtime, just verify data is present and can be parsed
    assert!(!prefetch_data.data.is_empty());
    let json_str = String::from_utf8(prefetch_data.data).unwrap();
    assert!(json_str.contains(r#""name":"crate""#));
}

#[db_test]
async fn get_prefetch_data_with_full_data(test_db: &kellnr_db::Database) {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    let created3 = Utc.with_ymd_and_hms(2020, 10, 9, 13, 18, 00).unwrap();
    let pm1 = PublishMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep1".to_string(),
            version_req: "1.0.0".to_string(),
            features: Some(vec!["feat1".to_string(), "feat2".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind1".to_string()),
            registry: Some("registry1".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        homepage: Some("homepage1".to_string()),
        documentation: Some("documentation1".to_string()),
        readme: Some("readme1".to_string()),
        repository: Some("repository1".to_string()),
        keywords: vec!["keyword1".to_string()],
        categories: vec!["category1".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
        readme_file: Some("readme_file1".to_string()),
        rust_version: None,
    };
    let pm2 = PublishMetadata {
        name: "crate".to_string(),
        vers: "2.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep2".to_string(),
            version_req: "2.0.0".to_string(),
            features: Some(vec!["feat2".to_string(), "feat3".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind2".to_string()),
            registry: Some("registry2".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature3".to_string(), vec!["dep2".to_string()]),
            ("feature4".to_string(), vec!["dep2".to_string()]),
        ]),
        authors: Some(vec!["author2".to_string(), "author3".to_string()]),
        description: Some("description2".to_string()),
        homepage: Some("homepage2".to_string()),
        documentation: Some("documentation2".to_string()),
        readme: Some("readme2".to_string()),
        repository: Some("repository2".to_string()),
        keywords: vec!["keyword2".to_string()],
        categories: vec!["category2".to_string()],
        license: Some("license2".to_string()),
        license_file: Some("license_file2".to_string()),
        badges: None,
        links: Some("links2".to_string()),
        readme_file: Some("readme_file2".to_string()),
        rust_version: None,
    };
    let pm3 = PublishMetadata {
        name: "crate".to_string(),
        vers: "3.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep3".to_string(),
            version_req: "3.0.0".to_string(),
            features: Some(vec!["feat3".to_string(), "feat4".to_string()]),
            optional: true,
            default_features: false,
            target: Some("target3".to_string()),
            kind: Some("kind3".to_string()),
            registry: Some("registry3".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature5".to_string(), vec!["dep3".to_string()]),
            ("feature6".to_string(), vec!["dep3".to_string()]),
        ]),
        authors: Some(vec!["author3".to_string(), "author4".to_string()]),
        description: Some("description3".to_string()),
        homepage: Some("homepage3".to_string()),
        documentation: Some("documentation3".to_string()),
        readme: Some("readme3".to_string()),
        repository: Some("repository3".to_string()),
        keywords: vec!["keyword3".to_string()],
        categories: vec!["category3".to_string()],
        license: Some("license3".to_string()),
        license_file: Some("license_file3".to_string()),
        badges: None,
        links: Some("links3".to_string()),
        readme_file: Some("readme_file3".to_string()),
        rust_version: None,
    };

    test_db
        .add_crate(&pm1, "cksum1_1", 1024, &created1, "admin", None)
        .await
        .unwrap();
    test_db
        .add_crate(&pm2, "cksum2_1", 1024, &created2, "admin", None)
        .await
        .unwrap();
    test_db
        .add_crate(&pm3, "cksum3_1", 1024, &created3, "admin", None)
        .await
        .unwrap();

    let prefetch_data = test_db.get_prefetch_data("crate").await.unwrap();

    // data length and etag vary with pubtime (set to Utc::now()), just verify data is present
    assert!(!prefetch_data.data.is_empty());
    assert_eq!(
        created3.format("%Y-%m-%d %H:%M:%S").to_string(),
        prefetch_data.last_modified
    );
    // etag changes with pubtime, so just verify it's not empty
    assert!(!prefetch_data.etag.is_empty());
    // Verify the data contains expected crate info
    let json_str = String::from_utf8(prefetch_data.data).unwrap();
    assert!(json_str.contains(r#""name":"crate""#));
    assert!(json_str.contains(r#""vers":"1.0.0""#));
    assert!(json_str.contains(r#""vers":"2.0.0""#));
    assert!(json_str.contains(r#""vers":"3.0.0""#));
}

#[db_test]
async fn delete_updates_etag(test_db: &kellnr_db::Database) {
    let created1 = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let created2 = Utc.with_ymd_and_hms(2020, 10, 8, 13, 18, 00).unwrap();
    let pm1 = PublishMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep1".to_string(),
            version_req: "1.0.0".to_string(),
            features: Some(vec!["feat1".to_string(), "feat2".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind1".to_string()),
            registry: Some("registry1".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature1".to_string(), vec!["dep1".to_string()]),
            ("feature2".to_string(), vec!["dep1".to_string()]),
        ]),
        authors: Some(vec!["author1".to_string(), "author2".to_string()]),
        description: Some("description1".to_string()),
        homepage: Some("homepage1".to_string()),
        documentation: Some("documentation1".to_string()),
        readme: Some("readme1".to_string()),
        repository: Some("repository1".to_string()),
        keywords: vec!["keyword1".to_string()],
        categories: vec!["category1".to_string()],
        license: Some("license1".to_string()),
        license_file: Some("license_file1".to_string()),
        badges: None,
        links: Some("links1".to_string()),
        readme_file: Some("readme_file1".to_string()),
        rust_version: None,
    };
    let pm2 = PublishMetadata {
        name: "crate".to_string(),
        vers: "2.0.0".to_string(),
        deps: vec![RegistryDep {
            name: "dep2".to_string(),
            version_req: "2.0.0".to_string(),
            features: Some(vec!["feat2".to_string(), "feat3".to_string()]),
            optional: true,
            default_features: false,
            target: Some("normal".to_string()),
            kind: Some("kind2".to_string()),
            registry: Some("registry2".to_string()),
            explicit_name_in_toml: None,
        }],
        features: BTreeMap::from_iter(vec![
            ("feature3".to_string(), vec!["dep2".to_string()]),
            ("feature4".to_string(), vec!["dep2".to_string()]),
        ]),
        authors: Some(vec!["author2".to_string(), "author3".to_string()]),
        description: Some("description2".to_string()),
        homepage: Some("homepage2".to_string()),
        documentation: Some("documentation2".to_string()),
        readme: Some("readme2".to_string()),
        repository: Some("repository2".to_string()),
        keywords: vec!["keyword2".to_string()],
        categories: vec!["category2".to_string()],
        license: Some("license2".to_string()),
        license_file: Some("license_file2".to_string()),
        badges: None,
        links: Some("links2".to_string()),
        readme_file: Some("readme_file2".to_string()),
        rust_version: None,
    };
    test_db
        .add_crate(&pm1, "cksum1_1", 1024, &created1, "admin", None)
        .await
        .unwrap();
    test_db
        .add_crate(&pm2, "cksum2_1", 1024, &created2, "admin", None)
        .await
        .unwrap();
    let prefetch_data = test_db.get_prefetch_data("crate").await.unwrap();
    let etag_before = prefetch_data.etag;

    test_db
        .delete_crate(
            &NormalizedName::from_unchecked("crate".to_string()),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();
    let prefetch_data = test_db.get_prefetch_data("crate").await.unwrap();
    let etag_after = prefetch_data.etag;

    assert_ne!(etag_before, etag_after);
}

#[db_test]
async fn is_cratesio_cache_up_to_date_not_found(test_db: &kellnr_db::Database) {
    let prefetch_state = test_db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("etag".to_string()),
            Some("last_modified".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(PrefetchState::NotFound, prefetch_state);
}

#[db_test]
async fn is_cratesio_cache_up_to_date_up_to_date(test_db: &kellnr_db::Database) {
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked("crate".to_string()),
            "etag",
            "last_modified",
            None,
            &[IndexMetadata {
                name: "crate".to_string(),
                vers: "1.0.0".to_string(),
                deps: vec![],
                cksum: "cksum".to_string(),
                features: BTreeMap::default(),
                yanked: false,
                links: None,
                pubtime: None,
                v: Some(1),
                features2: None,
                rust_version: None,
            }],
            &HashMap::new(),
        )
        .await
        .unwrap();

    let prefetch_state = test_db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("etag".to_string()),
            Some("last_modified".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(
        PrefetchState::UpToDate {
            etag: "etag".to_string(),
            last_modified: "last_modified".to_string()
        },
        prefetch_state
    );
}

#[db_test]
async fn is_cratesio_cache_up_to_date_needs_update(test_db: &kellnr_db::Database) {
    let indices1 = vec![IndexMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![],
        cksum: "cksum".to_string(),
        features: BTreeMap::default(),
        yanked: false,
        links: None,
        pubtime: None,
        v: Some(1),
        features2: None,
        rust_version: None,
    }];
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked("crate".to_string()),
            "etag",
            "last_modified",
            None,
            &indices1,
            &HashMap::new(),
        )
        .await
        .unwrap();
    let indices2 = vec![
        IndexMetadata {
            name: "crate".to_string(),
            vers: "1.0.0".to_string(),
            deps: vec![],
            cksum: "cksum".to_string(),
            features: BTreeMap::default(),
            yanked: true,
            links: None,
            pubtime: None,
            v: Some(1),
            features2: None,
            rust_version: None,
        },
        IndexMetadata {
            name: "crate".to_string(),
            vers: "2.0.0".to_string(),
            deps: vec![],
            cksum: "cksum".to_string(),
            features: BTreeMap::default(),
            yanked: false,
            links: None,
            pubtime: None,
            v: Some(1),
            features2: None,
            rust_version: None,
        },
    ];
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked("crate".to_string()),
            "etag2",
            "last_modified2",
            None,
            &indices2,
            &HashMap::new(),
        )
        .await
        .unwrap();

    let expected_prefetch = Prefetch {
        data: IndexMetadata::serialize_indices(&indices2)
            .unwrap()
            .into_bytes(),
        etag: "etag2".to_string(),
        last_modified: "last_modified2".to_string(),
    };

    // Old etag
    let prefetch_state = test_db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("old_etag".to_string()),
            Some("last_modified".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(
        PrefetchState::NeedsUpdate(expected_prefetch.clone()),
        prefetch_state
    );

    // Old last_modified
    let prefetch_state = test_db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("etag".to_string()),
            Some("old_last_modified".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(
        PrefetchState::NeedsUpdate(expected_prefetch.clone()),
        prefetch_state
    );

    // Old etag and last_modified
    let prefetch_state = test_db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("etag".to_string()),
            Some("old_last_modified".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(
        PrefetchState::NeedsUpdate(expected_prefetch),
        prefetch_state
    );
}

#[db_test]
async fn un_yank_crate(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    test_add_crate(
        test_db,
        "crate",
        "admin",
        &Version::from_unchecked_str("1.0.0"),
        &created,
    )
    .await
    .unwrap();

    // Yank the version
    test_db
        .yank_crate(
            &NormalizedName::from_unchecked_str("crate"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    // Test if yanked
    let ci = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("crate"))
        .await
        .unwrap();
    assert!(
        ci.versions
            .iter()
            .find(|v| v.version == "1.0.0")
            .unwrap()
            .yanked
    );

    // Unyank the version
    test_db
        .unyank_crate(
            &NormalizedName::from_unchecked_str("crate"),
            &Version::from_unchecked_str("1.0.0"),
        )
        .await
        .unwrap();

    // Test if unyanked
    let ci = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("crate"))
        .await
        .unwrap();
    assert!(
        !ci.versions
            .iter()
            .find(|v| v.version == "1.0.0")
            .unwrap()
            .yanked
    );
}

#[db_test]
async fn test_get_last_updated_crate_works(test_db: &kellnr_db::Database) {
    let created1 = DateTime::parse_from_rfc3339("2021-01-01T00:00:00Z").unwrap();
    let created1 = DateTime::<Utc>::from(created1);

    test_add_crate(
        test_db,
        "my_crate",
        "admin",
        &Version::from_unchecked_str("1.0.0"),
        &created1,
    )
    .await
    .unwrap();

    let created2 = DateTime::parse_from_rfc3339("2021-02-01T00:00:00Z").unwrap();
    let created2 = DateTime::<Utc>::from(created2);

    test_add_crate(
        test_db,
        "my_crate",
        "admin",
        &Version::from_unchecked_str("2.0.0"),
        &created2,
    )
    .await
    .unwrap();

    let created3 = DateTime::parse_from_rfc3339("2021-03-01T00:00:00Z").unwrap();
    let created3 = DateTime::<Utc>::from(created3);

    test_add_crate(
        test_db,
        "my_crate2",
        "admin",
        &Version::from_unchecked_str("1.0.0"),
        &created3,
    )
    .await
    .unwrap();

    let last_updated = test_db
        .get_last_updated_crate(&CrateVisibility::All)
        .await
        .unwrap()
        .unwrap();

    assert_eq!("my_crate2", last_updated.0.to_string());
}

#[db_test]
async fn test_get_last_updated_crate_empty(test_db: &kellnr_db::Database) {
    let last_updated = test_db
        .get_last_updated_crate(&CrateVisibility::All)
        .await
        .unwrap();

    assert_eq!(None, last_updated);
}

#[db_test]
async fn test_get_total_unique_cached_crates_works(test_db: &kellnr_db::Database) {
    test_add_cached_crate(test_db, "my_crate", "1.0.0")
        .await
        .unwrap();

    test_add_cached_crate(test_db, "my_crate", "2.0.0")
        .await
        .unwrap();

    test_add_cached_crate(test_db, "my_crate2", "1.0.0")
        .await
        .unwrap();

    let unique_cached_crates = test_db.get_total_unique_cached_crates().await.unwrap();

    assert_eq!(2, unique_cached_crates);
}

#[db_test]
async fn test_get_total_cached_crate_versions_works(test_db: &kellnr_db::Database) {
    test_add_cached_crate(test_db, "my_crate", "1.0.0")
        .await
        .unwrap();

    test_add_cached_crate(test_db, "my_crate", "2.0.0")
        .await
        .unwrap();

    test_add_cached_crate(test_db, "my_crate2", "1.0.0")
        .await
        .unwrap();

    let unique_cached_versions = test_db.get_total_cached_crate_versions().await.unwrap();

    assert_eq!(3, unique_cached_versions);
}

#[db_test]
async fn test_get_total_cached_downloads_works(test_db: &kellnr_db::Database) {
    test_add_cached_crate_with_downloads(test_db, "my_crate", "1.0.0", 10)
        .await
        .unwrap();

    test_add_cached_crate_with_downloads(test_db, "my_crate", "2.0.0", 20)
        .await
        .unwrap();

    test_add_cached_crate_with_downloads(test_db, "my_crate2", "1.0.0", 30)
        .await
        .unwrap();

    let total_downloads = test_db.get_total_cached_downloads().await.unwrap();

    assert_eq!(60, total_downloads);
}

#[db_test]
async fn test_add_crate_rollback(test_db: &kellnr_db::Database) {
    test_add_crate(
        test_db,
        "mycrate",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc::now(),
    )
    .await
    .unwrap();

    let result = test_add_crate(
        test_db,
        "mycrate",
        "user",
        &Version::try_from("2.0.0").unwrap(),
        &Utc::now(),
    )
    .await;
    // The result is err, as the `user` does not exist.
    assert!(result.is_err());

    let max_version = test_db
        .get_max_version_from_name(&NormalizedName::from_unchecked("mycrate".to_string()))
        .await
        .unwrap();

    // The version should not be bumped to 2.0.0 as the db transaction is not committed.
    assert_eq!("1.0.0", max_version.to_string());
}

#[db_test]
async fn test_delete_crate_rollback(test_db: &kellnr_db::Database) {
    let version = Version::try_from("1.0.0").unwrap();
    let crate_id = test_add_crate(test_db, "mycrate", "admin", &version, &Utc::now())
        .await
        .unwrap();

    // Manually delete crate index entry so the crate delete method fails in the middle.
    test_delete_crate_index(test_db, crate_id).await.unwrap();

    let normalized_name = NormalizedName::from_unchecked("mycrate".to_string());
    let result = test_db.delete_crate(&normalized_name, &version).await;
    assert!(result.is_err());

    let meta = test_db.get_crate_meta_list(&normalized_name).await.unwrap();
    // Crate meta is deleted first, but the actions should be rolled back on error.
    assert_eq!(1, meta.len());
}

#[db_test]
async fn test_register_webhook(test_db: &kellnr_db::Database) {
    let webhook = Webhook {
        id: None,
        event: WebhookEvent::CrateYank,
        callback_url: "https://my-other-service:8005".to_string(),
        name: Some("myWebhook".to_string()),
        secret: Some("whsec_c2VjcmV0".to_string()),
    };
    let id = test_db.register_webhook(webhook).await.unwrap();
    let entry = test_db.get_webhook(&id).await.unwrap();

    assert_eq!(WebhookEvent::CrateYank, entry.event);
    assert_eq!(
        "https://my-other-service:8005".to_string(),
        entry.callback_url
    );
    assert_eq!(Some("myWebhook".to_string()), entry.name);
    assert_eq!(Some("whsec_c2VjcmV0".to_string()), entry.secret);
}

#[db_test]
async fn test_delete_webhook(test_db: &kellnr_db::Database) {
    let webhook = Webhook {
        id: None,
        event: WebhookEvent::CrateYank,
        callback_url: "https://my-other-service:8005".to_string(),
        name: None,
        secret: None,
    };
    let id = test_db.register_webhook(webhook).await.unwrap();

    let result = test_db.delete_webhook(&id).await;
    assert!(result.is_ok());

    let result = test_db.get_webhook(&id).await;
    assert!(result.is_err());
}

#[db_test]
async fn test_get_all_webhooks(test_db: &kellnr_db::Database) {
    let mut ids = vec![];
    for i in 0..10 {
        ids.push(
            test_db
                .register_webhook(Webhook {
                    id: None,
                    event: if i % 2 == 0 {
                        WebhookEvent::CrateYank
                    } else {
                        WebhookEvent::CrateAdd
                    },
                    callback_url: String::new(),
                    name: None,
                    secret: None,
                })
                .await
                .unwrap(),
        );
    }

    let entries = test_db.get_all_webhooks().await.unwrap();

    assert_eq!(10, entries.len());
    for id in &ids {
        assert!(entries.iter().any(|a| a.id.as_ref().unwrap() == id));
    }
}

#[db_test]
async fn test_add_webhook_queue(test_db: &kellnr_db::Database) {
    let webhook = Webhook {
        id: None,
        event: WebhookEvent::CrateUpdate,
        callback_url: "https://update-service.io".to_string(),
        name: None,
        secret: Some("whsec_c2VjcmV0".to_string()),
    };
    let webhook_id = test_db.register_webhook(webhook).await.unwrap();

    let payload = json!({"test": "data"});
    let result = test_db
        .add_webhook_queue(WebhookEvent::CrateUpdate, payload.clone())
        .await;
    assert!(result.is_ok());

    // New entry should always be `pending`
    let entries = test_db
        .get_pending_webhook_queue_entries(Utc::now())
        .await
        .unwrap();

    assert_eq!(entries[0].payload, payload);
    assert_eq!(
        entries[0].callback_url,
        "https://update-service.io".to_string()
    );
    assert_eq!(entries[0].last_attempt, None);
    assert_eq!(entries[0].secret, Some("whsec_c2VjcmV0".to_string()));
}

#[db_test]
async fn test_get_pending_webhook_queue_entries(test_db: &kellnr_db::Database) {
    let webhook = Webhook {
        id: None,
        event: WebhookEvent::CrateUpdate,
        callback_url: String::new(),
        name: None,
        secret: None,
    };
    let webhook_id = test_db.register_webhook(webhook).await.unwrap();

    let payloads = (0..3).map(|i| json!(i)).collect::<Vec<_>>();
    for payload in &payloads {
        test_db
            .add_webhook_queue(WebhookEvent::CrateUpdate, payload.clone())
            .await
            .unwrap();
    }

    let entries = test_db
        .get_pending_webhook_queue_entries(Utc::now())
        .await
        .unwrap();

    // Do not rely on indexing as DB order is not deterministic
    let entry_1 = entries.iter().find(|a| a.payload == json!(1)).unwrap();

    test_db
        .update_webhook_queue(
            &entry_1.id,
            entry_1.next_attempt,
            Utc::now() + TimeDelta::minutes(15),
        )
        .await
        .unwrap();

    // Schedule one of the entries for a future time.
    let entries = test_db
        .get_pending_webhook_queue_entries(Utc::now())
        .await
        .unwrap();

    assert_eq!(2, entries.len());
    assert!(!entries.iter().any(|a| a.payload == json!(1)));
}

#[db_test]
async fn test_delete_webhook_queue(test_db: &kellnr_db::Database) {
    let webhook = Webhook {
        id: None,
        event: WebhookEvent::CrateUpdate,
        callback_url: String::new(),
        name: None,
        secret: None,
    };
    let webhook_id = test_db.register_webhook(webhook).await.unwrap();

    let payload = json!(0);
    let result = test_db
        .add_webhook_queue(WebhookEvent::CrateUpdate, payload.clone())
        .await;
    assert!(result.is_ok());

    let entry = &test_db
        .get_pending_webhook_queue_entries(Utc::now())
        .await
        .unwrap()[0];

    test_db.delete_webhook_queue(&entry.id).await.unwrap();
    let entries = test_db
        .get_pending_webhook_queue_entries(Utc::now())
        .await
        .unwrap();
    assert!(entries.is_empty());
}

#[db_test]
async fn pubtime_stored_and_serialized_correctly(test_db: &kellnr_db::Database) {
    // Add a crate - pubtime will be set to current time by IndexMetadata::from_reg_meta
    let created = Utc.with_ymd_and_hms(2025, 1, 2, 9, 5, 7).unwrap();
    test_add_crate(
        test_db,
        "testcrate",
        "admin",
        &Version::from_unchecked_str("1.0.0"),
        &created,
    )
    .await
    .unwrap();

    // Retrieve the prefetch data
    let prefetch_data = test_db.get_prefetch_data("testcrate").await.unwrap();

    // Convert the bytes to string
    let json_str = String::from_utf8(prefetch_data.data).unwrap();

    // Verify pubtime is present in JSON
    assert!(
        json_str.contains(r#""pubtime":""#),
        "Expected pubtime field to be present in JSON: {json_str}"
    );

    // Verify pubtime does NOT contain fractional seconds (no dot after seconds)
    // Valid: "pubtime":"2025-01-02T09:05:07Z"
    // Invalid: "pubtime":"2025-01-02T09:05:07.123456Z"
    assert!(
        !json_str.contains(r".000") || !json_str.contains("pubtime"),
        "pubtime should not contain fractional seconds, got: {json_str}"
    );

    // Deserialize and verify the pubtime value is preserved
    let deserialized: IndexMetadata = serde_json::from_str(&json_str).unwrap();
    assert!(
        deserialized.pubtime.is_some(),
        "Expected pubtime to be Some after deserialization"
    );

    // Verify the deserialized pubtime can be formatted back to the same format
    let pubtime = deserialized.pubtime.unwrap();
    let formatted = pubtime.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    assert!(
        json_str.contains(&format!(r#""pubtime":"{formatted}""#)),
        "Expected pubtime '{formatted}' in JSON: {json_str}"
    );
}

#[db_test]
async fn cratesio_index_keeps_rust_version(test_db: &kellnr_db::Database) {
    let indices = vec![IndexMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![],
        cksum: "cksum".to_string(),
        features: BTreeMap::default(),
        yanked: false,
        links: None,
        pubtime: None,
        v: Some(2),
        features2: None,
        rust_version: Some("1.85".to_string()),
    }];
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::from_unchecked("crate".to_string()),
            "etag",
            "last_modified",
            None,
            &indices,
            &HashMap::new(),
        )
        .await
        .unwrap();

    // An outdated etag forces the index to be rebuilt from the database
    // instead of being echoed back from the data passed in above.
    let prefetch_state = test_db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("old_etag".to_string()),
            None,
        )
        .await
        .unwrap();

    let PrefetchState::NeedsUpdate(prefetch) = prefetch_state else {
        panic!("Expected the cratesio cache to need an update: {prefetch_state:?}");
    };
    let data = String::from_utf8(prefetch.data).unwrap();
    let index: IndexMetadata = serde_json::from_str(data.lines().next().unwrap()).unwrap();

    assert_eq!(Some("1.85".to_string()), index.rust_version);
}

#[db_test]
async fn crate_index_keeps_rust_version(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let mut pm = PublishMetadata::minimal("crate", "1.0.0");
    pm.rust_version = Some("1.85".to_string());
    test_db
        .add_crate(&pm, "cksum", 1024, &created, "admin", None)
        .await
        .unwrap();

    let prefetch = test_db.get_prefetch_data("crate").await.unwrap();
    let data = String::from_utf8(prefetch.data).unwrap();
    let index: IndexMetadata = serde_json::from_str(data.lines().next().unwrap()).unwrap();

    assert_eq!(Some("1.85".to_string()), index.rust_version);
}

#[db_test]
async fn cratesio_index_heals_missing_rust_version(test_db: &kellnr_db::Database) {
    let mut index = IndexMetadata {
        name: "crate".to_string(),
        vers: "1.0.0".to_string(),
        deps: vec![],
        cksum: "cksum".to_string(),
        features: BTreeMap::default(),
        yanked: false,
        links: None,
        pubtime: None,
        v: Some(2),
        features2: None,
        rust_version: None,
    };
    let name = OriginalName::from_unchecked("crate".to_string());

    // A cache written before rust_version was known, e.g. by an older Kellnr.
    test_db
        .add_cratesio_prefetch_data(
            &name,
            "etag",
            "last_modified",
            None,
            &[index.clone()],
            &HashMap::new(),
        )
        .await
        .unwrap();

    // The same version comes back from the background refresh, now with an
    // MSRV. The row already exists, so only an update can heal it.
    index.rust_version = Some("1.85".to_string());
    test_db
        .add_cratesio_prefetch_data(
            &name,
            "new_etag",
            "last_modified",
            None,
            &[index],
            &HashMap::new(),
        )
        .await
        .unwrap();

    let prefetch_state = test_db
        .is_cratesio_cache_up_to_date(
            &NormalizedName::from(OriginalName::try_from("crate").unwrap()),
            Some("old_etag".to_string()),
            None,
        )
        .await
        .unwrap();

    let PrefetchState::NeedsUpdate(prefetch) = prefetch_state else {
        panic!("Expected the cratesio cache to need an update: {prefetch_state:?}");
    };
    let data = String::from_utf8(prefetch.data).unwrap();
    let served: IndexMetadata = serde_json::from_str(data.lines().next().unwrap()).unwrap();

    assert_eq!(Some("1.85".to_string()), served.rust_version);
}

#[db_test]
async fn has_oauth2_admin_requires_an_admin_with_a_linked_identity(test_db: &kellnr_db::Database) {
    // A local-only admin does not count. It cannot log in once SSO is enforced,
    // which is exactly the lockout this check exists to detect.
    test_db
        .add_user("local_admin", "pwd", true, false)
        .await
        .unwrap();
    assert!(!test_db.has_oauth2_admin().await.unwrap());

    // An SSO user without admin rights does not count either.
    test_db
        .create_oauth2_user("sso_user", "https://idp", "sub-1", None, false, false)
        .await
        .unwrap();
    assert!(!test_db.has_oauth2_admin().await.unwrap());

    // An admin with a linked identity is what makes the deployment usable.
    test_db
        .create_oauth2_user("sso_admin", "https://idp", "sub-2", None, true, false)
        .await
        .unwrap();
    assert!(test_db.has_oauth2_admin().await.unwrap());
}

/// Adds the crates `public`, `restricted_owned`, `restricted_user`,
/// `restricted_group` and `restricted_other` where every restricted crate is
/// accessible to `user` through a different path except `restricted_other`.
async fn add_crates_with_restrictions(test_db: &kellnr_db::Database) {
    test_db.add_user("user", "pwd", false, false).await.unwrap();
    test_db
        .add_user("other", "pwd", false, false)
        .await
        .unwrap();
    test_db.add_group("group").await.unwrap();
    test_db.add_group_user("group", "user").await.unwrap();

    let crates = [
        ("public", "admin", 1),
        ("restricted_owned", "user", 2),
        ("restricted_user", "admin", 3),
        ("restricted_group", "admin", 4),
        ("restricted_other", "other", 5),
    ];
    for (name, owner, day) in crates {
        let created = Utc.with_ymd_and_hms(2021, 1, day, 0, 0, 0).unwrap();
        test_add_crate(
            test_db,
            name,
            owner,
            &Version::try_from("1.0.0").unwrap(),
            &created,
        )
        .await
        .unwrap();
        test_db
            .increase_download_counter_by(
                &NormalizedName::from_unchecked(name.to_string()),
                &Version::try_from("1.0.0").unwrap(),
                u64::from(day) * 10,
            )
            .await
            .unwrap();
        if name != "public" {
            test_db
                .change_download_restricted(&NormalizedName::from_unchecked(name.to_string()), true)
                .await
                .unwrap();
        }
    }
    test_db
        .add_crate_user(
            &NormalizedName::from_unchecked("restricted_user".to_string()),
            "user",
        )
        .await
        .unwrap();
    test_db
        .add_crate_group(
            &NormalizedName::from_unchecked("restricted_group".to_string()),
            "group",
        )
        .await
        .unwrap();
}

async fn overview_names(
    test_db: &kellnr_db::Database,
    visibility: &CrateVisibility,
) -> Vec<String> {
    test_db
        .get_crate_overview_list(
            100,
            0,
            false,
            visibility,
            CrateSort::Name,
            &CrateTagFilter::default(),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect()
}

#[db_test]
async fn crate_overview_list_respects_visibility(test_db: &kellnr_db::Database) {
    add_crates_with_restrictions(test_db).await;

    assert_eq!(
        vec!["public"],
        overview_names(test_db, &CrateVisibility::Unrestricted).await
    );
    assert_eq!(
        vec![
            "public",
            "restricted_group",
            "restricted_owned",
            "restricted_user"
        ],
        overview_names(test_db, &CrateVisibility::User("user".to_string())).await
    );
    assert_eq!(
        vec!["public", "restricted_other"],
        overview_names(test_db, &CrateVisibility::User("other".to_string())).await
    );
    assert_eq!(
        vec!["public"],
        overview_names(test_db, &CrateVisibility::User("unknown".to_string())).await
    );
    assert_eq!(
        5,
        overview_names(test_db, &CrateVisibility::All).await.len()
    );
}

#[db_test]
async fn search_and_count_respect_visibility(test_db: &kellnr_db::Database) {
    add_crates_with_restrictions(test_db).await;
    let user = CrateVisibility::User("user".to_string());

    let found: Vec<String> = test_db
        .search_in_crate_name_and_description("restricted", 100, 0, false, &user)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(
        vec!["restricted_group", "restricted_owned", "restricted_user"],
        found
    );
    assert_eq!(
        3,
        test_db
            .count_by_crate_name_and_description("restricted", false, &user)
            .await
            .unwrap()
    );
    assert_eq!(
        0,
        test_db
            .count_by_crate_name_and_description(
                "restricted",
                false,
                &CrateVisibility::Unrestricted
            )
            .await
            .unwrap()
    );
    assert_eq!(
        4,
        test_db
            .count_by_crate_name_and_description("restricted", false, &CrateVisibility::All)
            .await
            .unwrap()
    );
}

#[db_test]
async fn statistics_respect_visibility(test_db: &kellnr_db::Database) {
    add_crates_with_restrictions(test_db).await;

    let top = test_db
        .get_top_crates_downloads(3, &CrateVisibility::Unrestricted)
        .await
        .unwrap();
    assert_eq!(vec![("public".to_string(), 10)], top);
    let top = test_db
        .get_top_crates_downloads(1, &CrateVisibility::User("user".to_string()))
        .await
        .unwrap();
    assert_eq!(vec![("restricted_group".to_string(), 40)], top);

    let last = test_db
        .get_last_updated_crate(&CrateVisibility::Unrestricted)
        .await
        .unwrap()
        .unwrap();
    assert_eq!("public", last.0.to_string());
    let last = test_db
        .get_last_updated_crate(&CrateVisibility::All)
        .await
        .unwrap()
        .unwrap();
    assert_eq!("restricted_other", last.0.to_string());
}

fn upstream_index(vers: &str) -> IndexMetadata {
    IndexMetadata {
        name: "Upstream".to_string(),
        vers: vers.to_string(),
        deps: vec![],
        cksum: format!("cksum-{vers}"),
        features: BTreeMap::default(),
        yanked: false,
        links: None,
        pubtime: None,
        v: Some(1),
        features2: None,
        rust_version: None,
    }
}

async fn add_upstream(
    test_db: &kellnr_db::Database,
    etag: &str,
    versions: &[&str],
    licenses: &[(&str, &str)],
) {
    let indices: Vec<_> = versions.iter().map(|v| upstream_index(v)).collect();
    let licenses: HashMap<String, String> = licenses
        .iter()
        .map(|(v, l)| ((*v).to_string(), (*l).to_string()))
        .collect();
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::try_from("Upstream").unwrap(),
            etag,
            "last_modified",
            None,
            &indices,
            &licenses,
        )
        .await
        .unwrap();
}

async fn upstream_meta(
    test_db: &kellnr_db::Database,
    version: &str,
) -> Option<kellnr_db::provider::CratesioVersionMeta> {
    test_db
        .get_cratesio_version_meta(
            &NormalizedName::from_unchecked_str("upstream"),
            &Version::try_from(version).unwrap(),
        )
        .await
        .unwrap()
}

#[db_test]
async fn get_cratesio_version_meta_returns_checksum_and_license(test_db: &kellnr_db::Database) {
    add_upstream(
        test_db,
        "etag",
        &["1.0.0", "1.1.0", "1.2.0"],
        &[("1.0.0", "MIT OR Apache-2.0"), ("1.1.0", UNKNOWN_LICENSE)],
    )
    .await;

    assert_eq!(
        Some(kellnr_db::provider::CratesioVersionMeta {
            cksum: "cksum-1.0.0".to_string(),
            license: Some("MIT OR Apache-2.0".to_string()),
        }),
        upstream_meta(test_db, "1.0.0").await
    );
    // The crate declares no license
    assert_eq!(
        Some(UNKNOWN_LICENSE.to_string()),
        upstream_meta(test_db, "1.1.0").await.unwrap().license
    );
    // The license was not fetched, e.g. because the API call failed
    assert_eq!(
        Some(kellnr_db::provider::CratesioVersionMeta {
            cksum: "cksum-1.2.0".to_string(),
            license: None,
        }),
        upstream_meta(test_db, "1.2.0").await
    );
    assert_eq!(None, upstream_meta(test_db, "2.0.0").await);
}

#[db_test]
async fn cratesio_prefetch_fills_missing_licenses_of_cached_versions(
    test_db: &kellnr_db::Database,
) {
    // Cached before licenses were fetched, or while the API was down
    add_upstream(test_db, "etag", &["1.0.0", "1.1.0"], &[]).await;
    assert_eq!(None, upstream_meta(test_db, "1.0.0").await.unwrap().license);

    add_upstream(
        test_db,
        "new_etag",
        &["1.0.0", "1.1.0"],
        &[("1.0.0", "MIT")],
    )
    .await;

    assert_eq!(
        Some("MIT".to_string()),
        upstream_meta(test_db, "1.0.0").await.unwrap().license
    );
    assert_eq!(None, upstream_meta(test_db, "1.1.0").await.unwrap().license);
}

#[db_test]
async fn cratesio_prefetch_keeps_known_licenses(test_db: &kellnr_db::Database) {
    add_upstream(test_db, "etag", &["1.0.0"], &[("1.0.0", "MIT")]).await;

    add_upstream(test_db, "new_etag", &["1.0.0"], &[("1.0.0", "GPL-3.0")]).await;

    assert_eq!(
        Some("MIT".to_string()),
        upstream_meta(test_db, "1.0.0").await.unwrap().license
    );
}

#[db_test]
async fn get_crate_license_returns_license_of_version(test_db: &kellnr_db::Database) {
    let created = Utc.with_ymd_and_hms(2020, 10, 7, 13, 18, 00).unwrap();
    let name = NormalizedName::from_unchecked_str("licensed");
    let mut pm = PublishMetadata::minimal("licensed", "1.0.0");
    pm.license = Some("MIT".to_string());
    test_db
        .add_crate(&pm, "cksum", 1, &created, "admin", None)
        .await
        .unwrap();
    let pm = PublishMetadata::minimal("licensed", "2.0.0");
    test_db
        .add_crate(&pm, "cksum", 1, &created, "admin", None)
        .await
        .unwrap();

    let license = |v: &'static str| {
        let name = name.clone();
        async move {
            test_db
                .get_crate_license(&name, &Version::try_from(v).unwrap())
                .await
                .unwrap()
        }
    };

    assert_eq!(Some("MIT".to_string()), license("1.0.0").await);
    assert_eq!(None, license("2.0.0").await);
    assert_eq!(None, license("3.0.0").await);
}

#[db_test]
async fn add_crate_adds_version_to_mixed_case_crate(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    for vers in ["1.0.0", "1.1.0"] {
        test_db
            .add_crate(
                &PublishMetadata::minimal("MyCrate", vers),
                "cksum",
                1024,
                &created,
                "admin",
                None,
            )
            .await
            .unwrap();
    }

    let name = NormalizedName::from_unchecked_str("mycrate");
    assert_eq!(2, test_db.get_crate_meta_list(&name).await.unwrap().len());
    assert_eq!(
        Version::try_from("1.1.0").unwrap(),
        test_db.get_max_version_from_name(&name).await.unwrap()
    );
}

#[db_test]
async fn add_crate_rejects_non_owner_of_existing_crate(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    test_db
        .add_user("mallory", "pwd", false, false)
        .await
        .unwrap();
    test_db
        .add_crate(
            &PublishMetadata::minimal("taken", "1.0.0"),
            "cksum",
            1024,
            &created,
            "admin",
            None,
        )
        .await
        .unwrap();

    let result = test_db
        .add_crate(
            &PublishMetadata::minimal("Taken", "1.1.0"),
            "cksum",
            1024,
            &created,
            "mallory",
            None,
        )
        .await;

    assert!(matches!(
        result,
        Err(kellnr_db::error::DbError::NotOwner(_))
    ));
    let name = NormalizedName::from_unchecked_str("taken");
    assert!(!test_db.is_owner(&name, "mallory").await.unwrap());
    assert_eq!(1, test_db.get_crate_meta_list(&name).await.unwrap().len());
}

#[db_test]
async fn add_crate_rejects_name_differing_only_in_dash_and_underscore(
    test_db: &kellnr_db::Database,
) {
    let created = Utc::now();
    test_db
        .add_crate(
            &PublishMetadata::minimal("foo-bar", "1.0.0"),
            "cksum",
            1024,
            &created,
            "admin",
            None,
        )
        .await
        .unwrap();

    for name in ["foo_bar", "Foo_Bar"] {
        let result = test_db
            .add_crate(
                &PublishMetadata::minimal(name, "1.0.0"),
                "cksum",
                1024,
                &created,
                "admin",
                None,
            )
            .await;
        assert!(
            matches!(result, Err(kellnr_db::error::DbError::CrateNameConflict(_, ref existing)) if existing == "foo-bar"),
            "{name}: {result:?}"
        );
    }
    assert!(
        test_db
            .get_crate_id(&NormalizedName::from_unchecked_str("foo_bar"))
            .await
            .unwrap()
            .is_none()
    );
}

#[db_test]
async fn add_empty_crate_rejects_similar_name(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    test_db.add_empty_crate("foo_bar", &created).await.unwrap();

    let result = test_db.add_empty_crate("foo-bar", &created).await;

    assert!(matches!(
        result,
        Err(kellnr_db::error::DbError::CrateNameConflict(_, _))
    ));
}

#[db_test]
async fn add_crate_keeps_existing_similar_names_publishable(test_db: &kellnr_db::Database) {
    // Registries created before the check may already hold both spellings.
    test_insert_crate_unchecked(test_db, "foo-bar")
        .await
        .unwrap();
    test_insert_crate_unchecked(test_db, "foo_bar")
        .await
        .unwrap();
    let created = Utc::now();

    for name in ["foo-bar", "foo_bar"] {
        test_db
            .add_crate(
                &PublishMetadata::minimal(name, "1.0.0"),
                "cksum",
                1024,
                &created,
                "admin",
                None,
            )
            .await
            .unwrap();
        let normalized = NormalizedName::from_unchecked_str(name);
        assert_eq!(
            1,
            test_db
                .get_crate_meta_list(&normalized)
                .await
                .unwrap()
                .len()
        );
    }
}

#[db_test]
async fn add_crate_allows_admin_on_existing_crate(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    test_db
        .add_user("alice", "pwd", false, false)
        .await
        .unwrap();
    test_db.add_user("root", "pwd", true, false).await.unwrap();
    test_db
        .add_crate(
            &PublishMetadata::minimal("owned", "1.0.0"),
            "cksum",
            1024,
            &created,
            "alice",
            None,
        )
        .await
        .unwrap();

    test_db
        .add_crate(
            &PublishMetadata::minimal("owned", "1.1.0"),
            "cksum",
            1024,
            &created,
            "root",
            None,
        )
        .await
        .unwrap();

    let name = NormalizedName::from_unchecked_str("owned");
    assert_eq!(2, test_db.get_crate_meta_list(&name).await.unwrap().len());
}

#[db_test]
async fn get_crate_dependents_uses_latest_version_of_dependents(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    let publish = async |name: &str, version: &str, deps: Vec<RegistryDep>| {
        let pm = PublishMetadata {
            deps,
            ..PublishMetadata::minimal(name, version)
        };
        test_db
            .add_crate(&pm, "cksum", 1024, &created, "admin", None)
            .await
            .unwrap();
    };
    let dep = |req: &str, kind: &str| RegistryDep {
        name: "foo".to_string(),
        version_req: req.to_string(),
        features: None,
        optional: false,
        default_features: true,
        target: None,
        kind: Some(kind.to_string()),
        registry: None,
        explicit_name_in_toml: None,
    };

    for version in ["1.0.0", "2.0.0", "3.0.0-beta.1"] {
        publish("foo", version, vec![]).await;
    }
    // Only the latest version of a dependent counts
    publish("App-Old", "1.0.0", vec![dep("^1", "normal")]).await;
    publish("app-new", "1.0.0", vec![dep("^1", "normal")]).await;
    publish(
        "app-new",
        "2.0.0",
        vec![
            dep("^2", "normal"),
            RegistryDep {
                explicit_name_in_toml: Some("foo_old".to_string()),
                ..dep("^1", "dev")
            },
        ],
    )
    .await;
    publish("app-moved-on", "1.0.0", vec![dep("^1", "normal")]).await;
    publish("app-moved-on", "2.0.0", vec![]).await;
    // Yanked versions do not count as the latest version
    publish("app-yanked", "1.0.0", vec![dep("^2", "build")]).await;
    publish("app-yanked", "2.0.0", vec![]).await;
    test_db
        .yank_crate(
            &NormalizedName::from_unchecked_str("app-yanked"),
            &Version::from_unchecked_str("2.0.0"),
        )
        .await
        .unwrap();
    // A crates.io crate with the same name is a different crate
    publish(
        "app-cratesio",
        "1.0.0",
        vec![RegistryDep {
            registry: Some("https://github.com/rust-lang/crates.io-index".to_string()),
            ..dep("^1", "normal")
        }],
    )
    .await;

    let cd = test_db
        .get_crate_dependents(&NormalizedName::from_unchecked_str("foo"))
        .await
        .unwrap();

    assert_eq!(Some("2.0.0".to_string()), cd.latest_version);
    assert_eq!(0, cd.restricted);
    // "name version: requirements", with "!" marking what is behind
    let summary: Vec<String> = cd
        .dependents
        .iter()
        .map(|d| {
            let reqs: Vec<String> = d
                .requirements
                .iter()
                .map(|r| {
                    format!(
                        "{} {}{}",
                        r.version_req,
                        r.kind,
                        if r.outdated { "!" } else { "" }
                    )
                })
                .collect();
            let behind = if d.outdated { "!" } else { "" };
            format!("{} {}{behind}: {}", d.name, d.version, reqs.join(", "))
        })
        .collect();
    assert_eq!(
        vec![
            "App-Old 1.0.0!: ^1 normal!",
            "app-new 2.0.0!: ^1 dev!, ^2 normal",
            "app-yanked 1.0.0: ^2 build",
        ],
        summary
    );
}

#[db_test]
async fn get_crate_dependents_of_unknown_crate_fails(test_db: &kellnr_db::Database) {
    let result = test_db
        .get_crate_dependents(&NormalizedName::from_unchecked_str("missing"))
        .await;
    assert!(matches!(
        result,
        Err(kellnr_db::error::DbError::CrateNotFound(_))
    ));
}

/// Publishes `name` in `version`, depending on "foo" with `req` if given
async fn publish_with_foo_dep(
    test_db: &kellnr_db::Database,
    name: &str,
    version: &str,
    req: Option<&str>,
) {
    let deps = req
        .map(|req| RegistryDep {
            name: "foo".to_string(),
            version_req: req.to_string(),
            features: None,
            optional: false,
            default_features: true,
            target: None,
            kind: Some("normal".to_string()),
            registry: None,
            explicit_name_in_toml: None,
        })
        .into_iter()
        .collect();
    let pm = PublishMetadata {
        deps,
        ..PublishMetadata::minimal(name, version)
    };
    test_db
        .add_crate(&pm, "cksum", 1024, &Utc::now(), "admin", None)
        .await
        .unwrap();
}

/// Name and version of the dependents of "foo"
async fn dependents_of_foo(test_db: &kellnr_db::Database) -> Vec<(String, String)> {
    test_db
        .get_crate_dependents(&NormalizedName::from_unchecked_str("foo"))
        .await
        .unwrap()
        .dependents
        .into_iter()
        .map(|d| (d.name, d.version))
        .collect()
}

#[db_test]
async fn get_crate_dependents_follows_deleted_and_yanked_versions(test_db: &kellnr_db::Database) {
    publish_with_foo_dep(test_db, "foo", "1.0.0", None).await;
    publish_with_foo_dep(test_db, "bar", "1.0.0", Some("^1")).await;
    publish_with_foo_dep(test_db, "bar", "2.0.0", None).await;
    assert!(dependents_of_foo(test_db).await.is_empty());

    // With the latest version yanked, the one before counts again
    let bar = NormalizedName::from_unchecked_str("bar");
    let v2 = Version::from_unchecked_str("2.0.0");
    test_db.yank_crate(&bar, &v2).await.unwrap();
    assert_eq!(
        vec![("bar".to_string(), "1.0.0".to_string())],
        dependents_of_foo(test_db).await
    );
    test_db.unyank_crate(&bar, &v2).await.unwrap();

    // The same after the latest version is deleted
    test_db.delete_crate(&bar, &v2).await.unwrap();
    assert_eq!(
        vec![("bar".to_string(), "1.0.0".to_string())],
        dependents_of_foo(test_db).await
    );

    // Deleting the version removes its dependencies
    test_db
        .delete_crate(&bar, &Version::from_unchecked_str("1.0.0"))
        .await
        .unwrap();
    assert!(dependents_of_foo(test_db).await.is_empty());
}

#[db_test]
async fn migration_fills_dependencies_of_existing_versions(test_db: &kellnr_db::Database) {
    publish_with_foo_dep(test_db, "foo", "1.0.0", None).await;
    publish_with_foo_dep(test_db, "bar", "1.0.0", Some("^1")).await;
    publish_with_foo_dep(test_db, "baz", "0.1.0", Some("^0.9")).await;
    let before = dependents_of_foo(test_db).await;
    assert_eq!(2, before.len());

    test_refill_crate_dependencies(test_db).await.unwrap();

    assert_eq!(before, dependents_of_foo(test_db).await);
}

#[db_test]
async fn migration_skips_dependencies_that_cannot_be_read(test_db: &kellnr_db::Database) {
    publish_with_foo_dep(test_db, "foo", "1.0.0", None).await;
    publish_with_foo_dep(test_db, "bar", "1.0.0", Some("^1")).await;
    publish_with_foo_dep(test_db, "baz", "1.0.0", Some("^1")).await;
    // An invalid dependency next to a valid one, and deps that are no list
    test_set_index_deps(
        test_db,
        "bar",
        json!([
            {"name": "foo", "broken": true},
            {
                "name": "foo",
                "req": "^1",
                "features": [],
                "optional": false,
                "default_features": true,
                "target": null,
                "kind": "normal",
                "registry": null
            }
        ]),
    )
    .await
    .unwrap();
    test_set_index_deps(test_db, "baz", json!({"name": "foo"}))
        .await
        .unwrap();

    test_refill_crate_dependencies(test_db).await.unwrap();

    assert_eq!(
        vec![("bar".to_string(), "1.0.0".to_string())],
        dependents_of_foo(test_db).await
    );
}

async fn add_cached(
    test_db: &kellnr_db::Database,
    name: &str,
    last_modified: &str,
    description: Option<&str>,
) {
    let index = IndexMetadata {
        name: name.to_string(),
        ..upstream_index("1.0.0")
    };
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::try_from(name).unwrap(),
            "etag",
            last_modified,
            description.map(str::to_string),
            &[index],
            &HashMap::new(),
        )
        .await
        .unwrap();
}

async fn cached_overview(test_db: &kellnr_db::Database, sort: CrateSort) -> Vec<CrateOverview> {
    test_db
        .get_crate_overview_list(
            100,
            0,
            true,
            &CrateVisibility::All,
            sort,
            &CrateTagFilter::default(),
        )
        .await
        .unwrap()
}

#[db_test]
async fn crate_overview_sorts_cached_crates_by_their_last_modified_date(
    test_db: &kellnr_db::Database,
) {
    test_add_crate(
        test_db,
        "local",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &Utc.with_ymd_and_hms(2024, 5, 1, 0, 0, 0).unwrap(),
    )
    .await
    .unwrap();
    add_cached(test_db, "newer", "Thu, 01 Oct 2026 12:34:56 GMT", None).await;
    add_cached(test_db, "older", "Mon, 01 Jan 2018 00:00:00 GMT", None).await;
    add_cached(test_db, "unknown", "", None).await;

    let overview = cached_overview(test_db, CrateSort::Updated).await;

    let names: Vec<_> = overview.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(vec!["newer", "local", "older", "unknown"], names);
    assert_eq!("2026-10-01 12:34:56", overview[0].date);

    // A sync without a valid date keeps the known one
    add_cached(test_db, "newer", "", None).await;
    let overview = cached_overview(test_db, CrateSort::Updated).await;
    assert_eq!("2026-10-01 12:34:56", overview[0].date);
}

#[db_test]
async fn cratesio_prefetch_updates_description_of_cached_crate(test_db: &kellnr_db::Database) {
    let description = |overview: Vec<CrateOverview>| overview[0].description.clone();
    add_cached(test_db, "cached", "", None).await;

    add_cached(test_db, "cached", "", Some("new description")).await;
    assert_eq!(
        Some("new description".to_string()),
        description(cached_overview(test_db, CrateSort::Name).await)
    );

    // A sync without a description, e.g. as the API call failed, keeps it
    add_cached(test_db, "cached", "", None).await;
    assert_eq!(
        Some("new description".to_string()),
        description(cached_overview(test_db, CrateSort::Name).await)
    );
}

#[db_test]
async fn migration_sets_date_of_cached_crates(test_db: &kellnr_db::Database) {
    add_cached(test_db, "newer", "Thu, 01 Oct 2026 12:34:56 GMT", None).await;
    add_cached(test_db, "invalid", "last_modified", None).await;

    test_refill_cratesio_last_updated(test_db).await.unwrap();

    let dates: Vec<_> = cached_overview(test_db, CrateSort::Updated)
        .await
        .into_iter()
        .map(|c| (c.name, c.date))
        .collect();
    assert_eq!(
        vec![
            ("newer".to_string(), "2026-10-01 12:34:56".to_string()),
            ("invalid".to_string(), String::new()),
        ],
        dates
    );
}

#[db_test]
async fn get_crate_data_with_more_dependencies_than_bind_parameters(test_db: &kellnr_db::Database) {
    // Every version repeats the dependencies, together they exceed the bind
    // parameter limits of SQLite and PostgreSQL
    let deps: Vec<RegistryDep> = (0..35_000)
        .map(|i| RegistryDep {
            name: format!("dep-{i}"),
            version_req: "^1".to_string(),
            features: None,
            optional: false,
            default_features: true,
            target: None,
            kind: Some("normal".to_string()),
            registry: Some(kellnr_common::crate_data::CRATESIO_INDEX.to_string()),
            explicit_name_in_toml: None,
        })
        .collect();
    for version in ["1.0.0", "2.0.0"] {
        let pm = PublishMetadata {
            deps: deps.clone(),
            ..PublishMetadata::minimal("many_deps", version)
        };
        test_db
            .add_crate(&pm, "cksum", 1024, &Utc::now(), "admin", None)
            .await
            .unwrap();
    }
    add_cached(test_db, "dep-7", "", Some("cached dependency")).await;

    let data = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("many_deps"))
        .await
        .unwrap();

    let dep = data.versions[0]
        .dependencies
        .iter()
        .find(|d| d.name == "dep-7")
        .unwrap();
    assert_eq!(Some("cached dependency".to_string()), dep.description);
    assert_eq!(Some("1.0.0".to_string()), dep.latest_version);
}

async fn publish_with_metadata(
    test_db: &kellnr_db::Database,
    version: &str,
    text: &str,
) -> Result<i64, kellnr_db::error::DbError> {
    let pm = PublishMetadata {
        description: Some(text.to_string()),
        homepage: Some(format!("https://{text}.example.com")),
        repository: Some(format!("https://{text}.example.com/repo")),
        keywords: vec![text.to_string()],
        categories: vec![text.to_string()],
        authors: Some(vec![text.to_string()]),
        ..PublishMetadata::minimal("meta", version)
    };
    test_db
        .add_crate(&pm, "cksum", 1024, &Utc::now(), "admin", None)
        .await
}

async fn assert_crate_metadata(test_db: &kellnr_db::Database, max_version: &str, text: &str) {
    let data = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("meta"))
        .await
        .unwrap();
    assert_eq!(max_version, data.max_version);
    assert_eq!(Some(text.to_string()), data.description);
    assert_eq!(Some(format!("https://{text}.example.com")), data.homepage);
    assert_eq!(
        Some(format!("https://{text}.example.com/repo")),
        data.repository
    );
    assert_eq!(vec![text.to_string()], data.keywords);
    assert_eq!(vec![text.to_string()], data.categories);
    assert_eq!(vec![text.to_string()], data.authors);
}

#[db_test]
async fn add_crate_keeps_metadata_of_newest_version(test_db: &kellnr_db::Database) {
    publish_with_metadata(test_db, "2.0.0", "newest")
        .await
        .unwrap();

    // A fix release of an older version
    publish_with_metadata(test_db, "1.0.1", "older")
        .await
        .unwrap();
    assert_crate_metadata(test_db, "2.0.0", "newest").await;

    publish_with_metadata(test_db, "3.0.0", "latest")
        .await
        .unwrap();
    assert_crate_metadata(test_db, "3.0.0", "latest").await;
}

#[db_test]
async fn add_crate_rejects_invalid_version_of_new_crate(test_db: &kellnr_db::Database) {
    let result = publish_with_metadata(test_db, "not-a-version", "invalid").await;

    assert!(matches!(
        result,
        Err(kellnr_db::error::DbError::InvalidVersion(_))
    ));
    assert_eq!(0, test_db.get_total_unique_crates().await.unwrap());
}

#[db_test]
async fn add_crate_with_duplicate_keywords_categories_and_authors(test_db: &kellnr_db::Database) {
    let twice = |a: &str, b: &str| vec![a.to_string(), b.to_string(), a.to_string()];
    for name in ["first", "second"] {
        let pm = PublishMetadata {
            keywords: twice("keyword", "shared-keyword"),
            categories: twice("category", "shared-category"),
            authors: Some(twice("author", "shared-author")),
            ..PublishMetadata::minimal(name, "1.0.0")
        };
        test_db
            .add_crate(&pm, "cksum", 1024, &Utc::now(), "admin", None)
            .await
            .unwrap();
    }

    for name in ["first", "second"] {
        let data = test_db
            .get_crate_data(&NormalizedName::from_unchecked_str(name))
            .await
            .unwrap();
        let mut keywords = data.keywords;
        keywords.sort();
        let mut categories = data.categories;
        categories.sort();
        let mut authors = data.authors;
        authors.sort();
        assert_eq!(vec!["keyword", "shared-keyword"], keywords);
        assert_eq!(vec!["category", "shared-category"], categories);
        assert_eq!(vec!["author", "shared-author"], authors);
    }
}

#[db_test]
async fn migration_removes_duplicate_crate_tag_links(test_db: &kellnr_db::Database) {
    let pm = PublishMetadata {
        keywords: vec!["keyword".to_string()],
        categories: vec!["category".to_string()],
        authors: Some(vec!["author".to_string()]),
        ..PublishMetadata::minimal("tagged", "1.0.0")
    };
    test_db
        .add_crate(&pm, "cksum", 1024, &Utc::now(), "admin", None)
        .await
        .unwrap();

    test_duplicate_and_remove_crate_tag_links(test_db)
        .await
        .unwrap();

    let data = test_db
        .get_crate_data(&NormalizedName::from_unchecked_str("tagged"))
        .await
        .unwrap();
    assert_eq!(vec!["keyword"], data.keywords);
    assert_eq!(vec!["category"], data.categories);
    assert_eq!(vec!["author"], data.authors);
}

/// Only on PostgreSQL, as concurrent write transactions of SQLite fail with
/// "database is locked" before they get to the keywords.
#[tokio::test]
async fn postgres_concurrent_publishes_share_new_keywords() {
    use testcontainers::runners::AsyncRunner;

    let pg_container = image::Postgres::default().start().await.unwrap();
    let port = pg_container
        .get_host_port_ipv4(image::Postgres::PG_PORT)
        .await
        .unwrap();
    let admin = kellnr_db::AdminUser::new("123".to_string(), Some("token".to_string()));
    let con = kellnr_db::ConString::Postgres(kellnr_db::PgConString::new(
        "localhost",
        port,
        "kellnr",
        "admin",
        "admin",
        admin,
        std::time::Duration::from_secs(3600),
    ));
    let test_db = &kellnr_db::Database::new(&con, 10).await.unwrap();

    let publish = |name: &'static str| async move {
        let pm = PublishMetadata {
            keywords: vec!["new-keyword".to_string()],
            categories: vec!["new-category".to_string()],
            authors: Some(vec!["new-author".to_string()]),
            ..PublishMetadata::minimal(name, "1.0.0")
        };
        test_db
            .add_crate(&pm, "cksum", 1024, &Utc::now(), "admin", None)
            .await
    };

    let results = tokio::join!(
        publish("crate1"),
        publish("crate2"),
        publish("crate3"),
        publish("crate4"),
    );

    results.0.unwrap();
    results.1.unwrap();
    results.2.unwrap();
    results.3.unwrap();
}

#[db_test]
async fn yank_and_unyank_update_etag_of_crate_index(test_db: &kellnr_db::Database) {
    let name = NormalizedName::from_unchecked_str("yankee");
    let version = Version::try_from("1.0.0").unwrap();
    // The index stores the original name, which differs from the normalized one
    for vers in ["1.0.0", "2.0.0"] {
        let pm = PublishMetadata::minimal("Yankee", vers);
        test_db
            .add_crate(&pm, "cksum", 1024, &Utc::now(), "admin", None)
            .await
            .unwrap();
    }
    let published = test_db.get_prefetch_data("yankee").await.unwrap();

    test_db.yank_crate(&name, &version).await.unwrap();

    let yanked = test_db.get_prefetch_data("yankee").await.unwrap();
    assert_ne!(published.etag, yanked.etag);
    assert_eq!(sha256::digest(yanked.data.as_slice()), yanked.etag);
    let lines: Vec<IndexMetadata> = String::from_utf8(yanked.data)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let versions: Vec<_> = lines.iter().map(|l| (l.vers.as_str(), l.yanked)).collect();
    assert_eq!(vec![("1.0.0", true), ("2.0.0", false)], versions);

    test_db.unyank_crate(&name, &version).await.unwrap();

    let unyanked = test_db.get_prefetch_data("yankee").await.unwrap();
    assert_eq!(published.etag, unyanked.etag);
    assert_eq!(published.data, unyanked.data);
}

#[db_test]
async fn set_channel_moves_channel_to_toolchain(test_db: &kellnr_db::Database) {
    test_db
        .add_toolchain("rust", "1.0.0", "2024-01-01", Some("stable".to_string()))
        .await
        .unwrap();
    test_db
        .add_toolchain("rust", "1.1.0", "2024-02-01", None)
        .await
        .unwrap();

    test_db
        .set_channel("stable", "rust", "1.1.0")
        .await
        .unwrap();

    let channels = test_db.get_channels().await.unwrap();
    assert_eq!(1, channels.len());
    assert_eq!("stable", channels[0].name);
    assert_eq!("1.1.0", channels[0].version);
}

#[db_test]
async fn set_channel_of_unknown_toolchain_fails_and_keeps_channel(test_db: &kellnr_db::Database) {
    test_db
        .add_toolchain("rust", "1.0.0", "2024-01-01", Some("stable".to_string()))
        .await
        .unwrap();

    let result = test_db.set_channel("stable", "rust", "9.9.9").await;

    assert!(matches!(
        result,
        Err(kellnr_db::error::DbError::ToolchainNotFound(_, _))
    ));
    let channels = test_db.get_channels().await.unwrap();
    assert_eq!(1, channels.len());
    assert_eq!("1.0.0", channels[0].version);
}

#[db_test]
async fn admin_created_by_concurrent_start_is_kept(test_db: &kellnr_db::Database) {
    test_insert_admin_credentials_again(test_db).await.unwrap();

    assert_eq!(1, test_db.get_users().await.unwrap().len());
    test_db.authenticate_user("admin", "123").await.unwrap();
    test_db.get_user_from_token("token").await.unwrap();
    assert!(test_db.get_user_from_token("other_token").await.is_err());
}

#[db_test]
async fn total_downloads_of_empty_registry_are_zero(test_db: &kellnr_db::Database) {
    assert_eq!(0, test_db.get_total_downloads().await.unwrap());
    assert_eq!(0, test_db.get_total_cached_downloads().await.unwrap());
}

#[db_test]
async fn cratesio_prefetch_skips_invalid_versions_for_max_version(test_db: &kellnr_db::Database) {
    let indices = vec![
        upstream_index("1.0.0"),
        upstream_index("not-a-version"),
        upstream_index("1.2.0"),
    ];
    test_db
        .add_cratesio_prefetch_data(
            &OriginalName::try_from("Upstream").unwrap(),
            "etag",
            "",
            None,
            &indices,
            &HashMap::new(),
        )
        .await
        .unwrap();

    let overview = cached_overview(test_db, CrateSort::Name).await;
    assert_eq!("1.2.0", overview[0].version);
}

#[db_test]
async fn delete_crate_skips_invalid_versions_for_max_version(test_db: &kellnr_db::Database) {
    let created = Utc::now();
    let crate_id = test_add_crate(
        test_db,
        "invalid_meta",
        "admin",
        &Version::try_from("1.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate(
        test_db,
        "invalid_meta",
        "admin",
        &Version::try_from("2.0.0").unwrap(),
        &created,
    )
    .await
    .unwrap();
    test_add_crate_meta(test_db, crate_id, "not-a-version", &created, None)
        .await
        .unwrap();

    let name = NormalizedName::from_unchecked_str("invalid_meta");
    test_db
        .delete_crate(&name, &Version::try_from("2.0.0").unwrap())
        .await
        .unwrap();

    assert_eq!(
        Version::try_from("1.0.0").unwrap(),
        test_db.get_max_version_from_name(&name).await.unwrap()
    );
}
