use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Postgres error occurred: {0}")]
    PostgresError(#[from] sea_orm::DbErr), // TODO find a good way to remove "postgres" as dependency here
    #[error("Passwords did not match")]
    PasswordMismatch,
    #[error("Failed to get parent directory for index")]
    NoIndexParentDirectory,
    #[error("Failed to create database directory")]
    CreateDatabaseDirectoryError,
    #[error("Failed to initialize the database: {0}")]
    InitializationError(String),
    #[error("User not found: {0}")]
    UserNotFound(String),
    #[error("Owner not found: {0}")]
    OwnerNotFound(String),
    #[error("Crate not found: {0}")]
    CrateNotFound(String),
    #[error("Crate with not found with id: {0}")]
    CrateNotFoundWithId(i64),
    #[error("Failed to count crate versions")]
    FailedToCountCrateVersions,
    #[error("Failed to count total crate downloads")]
    FailedToCountTotalDownloads,
    #[error("Failed to get crate summary for crate {0}")]
    FailedToGetCrateSummary(String),
    #[error("Token not found")]
    TokenNotFound,
    #[error("Session not found")]
    SessionNotFound,
    #[error("Failed to count all unique crates")]
    FailedToCountCrates,
    #[error("Crate meta information for crate {0} version {1} not found")]
    CrateMetaNotFound(String, String),
    #[error("Failed to get max version of crate id {0}")]
    FailedToGetMaxVersionById(i64),
    #[error("Failed to get max version of crate {0}")]
    FailedToGetMaxVersionByName(String),
    #[error("Invalid crate version {0}")]
    InvalidVersion(String),
    #[error("Failed to convert data to json: {0}")]
    FailedToConvertToJson(String),
    #[error("Failed to convert data from json: {0}")]
    FailedToConvertFromJson(String),
    #[error("Crate index not found for crate {0} - version {1}")]
    CrateIndexNotFound(String, String),
    #[error("Invalid crate name {0}")]
    InvalidCrateName(String),
    #[error("Crates.io index data is missing for crate {0}")]
    MissingCratesIoIndexData(String),
}
