use chrono::DateTime;
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;
use tracing::info;

use crate::iden::CratesIoIden;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // When a cached crates.io crate was last updated, as "YYYY-MM-DD HH:MM:SS"
        // like the dates of the crates of the registry, so both sort together.
        // "last_modified" keeps the raw HTTP header for conditional requests to
        // crates.io. Empty if the date is unknown.
        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoIden::Table)
                    .add_column(
                        ColumnDef::new(CratesIoIden::LastUpdated)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        fill_last_updated(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoIden::Table)
                    .drop_column(CratesIoIden::LastUpdated)
                    .to_owned(),
            )
            .await
    }
}

/// Converts the value of an HTTP Last-Modified header, like
/// "Thu, 01 Oct 2026 12:00:00 GMT", into the date format of the database.
/// `None` if the value is not a valid date.
pub fn http_date_to_db_date(value: &str) -> Option<String> {
    DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|date| date.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string())
}

/// Number of crates read at once, to not load the whole cache into memory
const PAGE_SIZE: u64 = 1000;

/// Sets the date of all cached crates whose Last-Modified header is a valid date
pub async fn fill_last_updated<C: ConnectionTrait>(db: &C) -> Result<(), DbErr> {
    let mut last_id = 0;
    let mut count = 0;
    loop {
        let rows = db
            .query_all(
                Query::select()
                    .columns([CratesIoIden::Id, CratesIoIden::LastModified])
                    .from(CratesIoIden::Table)
                    .and_where(Expr::col(CratesIoIden::Id).gt(last_id))
                    .order_by(CratesIoIden::Id, Order::Asc)
                    .limit(PAGE_SIZE),
            )
            .await?;
        let Some(last) = rows.last() else {
            break;
        };
        last_id = last.try_get::<i64>("", "id")?;

        for row in &rows {
            let id: i64 = row.try_get("", "id")?;
            let last_modified: String = row.try_get("", "last_modified")?;
            let Some(last_updated) = http_date_to_db_date(&last_modified) else {
                continue;
            };
            db.execute(
                &Query::update()
                    .table(CratesIoIden::Table)
                    .value(CratesIoIden::LastUpdated, last_updated)
                    .and_where(Expr::col(CratesIoIden::Id).eq(id))
                    .to_owned(),
            )
            .await?;
            count += 1;
        }
    }
    info!("Set the last update of {count} cached crates.io crates");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::http_date_to_db_date;

    #[test]
    fn converts_http_date() {
        assert_eq!(
            Some("2026-10-01 12:34:56".to_string()),
            http_date_to_db_date("Thu, 01 Oct 2026 12:34:56 GMT")
        );
    }

    #[test]
    fn rejects_invalid_date() {
        assert_eq!(None, http_date_to_db_date(""));
        assert_eq!(None, http_date_to_db_date("last_modified"));
    }
}
