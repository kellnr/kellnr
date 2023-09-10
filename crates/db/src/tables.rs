use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

pub async fn init_database(connection_string: impl ToString) -> Result<DatabaseConnection, DbErr> {
    let opt = ConnectOptions::new(connection_string.to_string());
    let db = Database::connect(opt).await?;

    Migrator::up(&db, None).await?;
    Ok(db)
}
