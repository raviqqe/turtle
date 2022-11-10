use async_trait::async_trait;
use std::error::Error;
use std::fmt::{self, Display};
use std::fmt::{Debug, Formatter};

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime};
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;

#[async_trait]
pub trait FileSystem: Debug {
    async fn read_file(&self, path: &Path, buffer: &mut Vec<u8>) -> Result<(), Box<dyn Error>>;
    async fn exists(&self, path: &Path) -> Result<(), Box<dyn Error>>;
    async fn modified_time(&self, path: &Path) -> Result<SystemTime, Box<dyn Error>>;
    async fn create_directory(&self, path: &Path) -> Result<(), Box<dyn Error>>;
    async fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, Box<dyn Error>>;
}

#[derive(Debug, Default)]
pub struct OsFileSystem {}

impl OsFileSystem {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl FileSystem for OsFileSystem {
    async fn read_file(&self, path: &Path, buffer: &mut Vec<u8>) -> Result<(), Box<dyn Error>> {
        File::open(path)
            .await
            .map_err(|error| OsFileSystemError::new(error, path))?
            .read_to_end(buffer)
            .await
            .map_err(|error| OsFileSystemError::new(error, path))?;

        Ok(())
    }

    async fn exists(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        fs::metadata(path)
            .await
            .map_err(|error| OsFileSystemError::new(error, path))?;

        Ok(())
    }

    async fn modified_time(&self, path: &Path) -> Result<SystemTime, Box<dyn Error>> {
        Ok(fs::metadata(path)
            .await
            .map_err(|error| OsFileSystemError::new(error, path))?
            .modified()
            .map_err(|error| OsFileSystemError::new(error, path))?)
    }

    async fn create_directory(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(path)
            .await
            .map_err(|error| OsFileSystemError::new(error, path))?;

        Ok(())
    }

    async fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, Box<dyn Error>> {
        Ok(fs::canonicalize(path)
            .await
            .map_err(|error| OsFileSystemError::new(error, path))?)
    }
}

#[derive(Debug)]
pub struct OsFileSystemError {
    error: io::Error,
    path: String,
}

impl OsFileSystemError {
    pub fn new(error: io::Error, path: &Path) -> Self {
        Self {
            error,
            path: path.display().to_string(),
        }
    }
}

impl Error for OsFileSystemError {}

impl Display for OsFileSystemError {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "{}: {}", &self.error, &self.path)
    }
}
