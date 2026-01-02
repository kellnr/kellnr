use kellnr_migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

pub async fn init_database(
    connection_string: impl ToString,
    max_con: u32,
) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(connection_string.to_string());
    if max_con > 0 {
        opt.max_connections(max_con);
    }
    let db = Database::connect(opt).await?;

    Migrator::up(&db, None).await?;
    Ok(db)
}
