use std::{io::ErrorKind, path::PathBuf};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Failed to create crate_folder")]
    CreateCrateFolder(std::io::Error),
    #[error("Crate with version already exists: {0}-{1}")]
    CrateExists(String, String),
    #[error("Failed to read crate file {0:?} to calc cksum after {1} tries")]
    ReadCrateFileTries(PathBuf, usize),
    #[error("Failed to create file {0} on storage: {1:?}")]
    CreateFile(std::io::Error, PathBuf),
    #[error("Failed to write crate to file {0:?}: {1}")]
    WriteCrateFile(PathBuf, std::io::Error),
    #[error("Failed to open crate file {0:?} to calc cksum: {1}")]
    OpenCrateFileToCalckCksum(PathBuf, std::io::Error),
    #[error("Failed to read crate file {0:?} to calc cksum: {1}")]
    ReadCrateFileToCalcCksum(PathBuf, std::io::Error),
    #[error("Failed to create bin path for crate file {0:?}: {1}")]
    CreateBinPath(PathBuf, std::io::Error),
    #[error("Failed to create doc queue sub-path {0:?}: {1}")]
    CreateDocQueuePath(PathBuf, std::io::Error),
    #[error("Failed to remove file {0:?} from crate storage: {1}")]
    RemoveFile(PathBuf, std::io::Error),
    #[error("Failed to read crate metadata from file {0:?}: {1}")]
    ReadCrateMetadata(PathBuf, std::io::Error),
    #[error("File does not exist: {0:?}")]
    FileDoesNotExist(PathBuf),
    #[error("Failed to open file {0:?} on storage: {1}")]
    OpenFileOnStorage(PathBuf, std::io::Error),
    #[error("Failed to read file {0:?}: {1}")]
    ReadFile(PathBuf, std::io::Error),
    #[error("Failed to read from file handle: {0}")]
    ReadFileHandle(std::io::Error),
    #[error("Failed to flush file {0:?}: {1}")]
    FlushCrateFile(PathBuf, std::io::Error),
    #[error("Failed to init s3 storage. Reason: {0}")]
    S3StorageInitError(String),
    #[error("S3 Error... Reason: {0}")]
    S3GenericError(String)
}
