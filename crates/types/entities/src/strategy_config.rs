use alloy_primitives::Address;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs;

#[derive(Debug, Error)]
pub enum LoadConfigError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    TomlError(#[from] toml::de::Error),
}

pub trait StrategyConfig {
    /// If None is returned, the strategy will use a random signer in the swap router.
    fn eoa(&self) -> Option<Address>;
}

pub async fn load_from_file<C: DeserializeOwned>(file_path: PathBuf) -> Result<C, LoadConfigError> {
    let contents = fs::read_to_string(file_path).await?;
    let config: C = toml::from_str(&contents)?;
    Ok(config)
}
