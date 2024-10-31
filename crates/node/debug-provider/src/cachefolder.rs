use std::path::Path;

use alloy::primitives::B256;
use eyre::Result;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct CacheFolder {
    path: String,
}

impl CacheFolder {
    pub async fn new(path: &str) -> Self {
        if !Path::new(path).exists() {
            fs::create_dir_all(path).await.unwrap();
        }
        CacheFolder { path: path.to_string() }
    }

    pub async fn write(&self, method: String, index: B256, data: String) -> Result<()> {
        let file_path = format!("{}/{}_{}.json", self.path, method.to_lowercase(), index.to_string().strip_prefix("0x").unwrap());
        let mut file = fs::File::create(file_path).await?;
        file.write_all(data.as_bytes()).await?;
        Ok(())
    }

    pub async fn read(&self, method: String, index: B256) -> Result<String> {
        let file_path = format!("{}/{}_{}.json", self.path, method.to_lowercase(), index.to_string().strip_prefix("0x").unwrap());
        let mut file = fs::File::open(file_path).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;
        Ok(content)
    }
}
