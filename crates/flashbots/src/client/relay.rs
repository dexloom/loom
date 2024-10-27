use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::client::jsonrpc::{JsonRpcError, Request, Response};
use alloy_primitives::{hex, keccak256};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;
use reqwest::{Client, Error as ReqwestError};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tracing::{debug, trace};
use url::Url;

/// Configuration for a Flashbots relay.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RelayConfig {
    pub id: u16,
    pub name: String,
    pub url: String,
    pub no_sign: Option<bool>,
}

/// A Flashbots relay client.
///
/// The client automatically signs every request and sets the Flashbots
/// authorization header appropriately with the given signer.
///
/// **Note**: You probably do not want to use this directly, unless
/// you want to interact directly with the Relay. Most users should use
/// [`FlashbotsClient`](crate::FlashbotsClient) instead.
#[derive(Clone)]
pub struct Relay {
    id: Arc<AtomicU64>,
    client: Client,
    url: Url,
    signer: Option<PrivateKeySigner>,
}

/// Errors for relay requests.
#[derive(Debug, Error)]
pub enum RelayError {
    /// The request failed.
    #[error(transparent)]
    RequestError(#[from] ReqwestError),
    /// The request could not be parsed.
    #[error(transparent)]
    JsonRpcError(#[from] JsonRpcError),
    /// The request parameters were invalid.
    #[error("Client error: {text}")]
    ClientError { text: String },
    /// The request could not be serialized.
    #[error(transparent)]
    RequestSerdeJson(#[from] serde_json::Error),
    /// The request could not be signed.
    #[error(transparent)]
    SignerError(alloy_signer::Error),
    /// The response could not be deserialized.
    #[error("Deserialization error: {err}. Response: {text}")]
    ResponseSerdeJson { err: serde_json::Error, text: String },
}

impl Relay {
    /// Initializes a new relay client.
    pub fn new(url: impl Into<Url>, signer: Option<PrivateKeySigner>) -> Self {
        //let client = Client::builder().trust_dns(true).build().unwrap();
        let client = Client::new();

        Self { id: Arc::new(AtomicU64::new(0)), client, url: url.into(), signer }
    }

    /// Sends a request with the provided method to the relay, with the
    /// parameters serialized as JSON.
    pub async fn request<T: Serialize + Send + Sync, R: DeserializeOwned>(&self, method: &str, params: T) -> Result<R, RelayError> {
        let next_id = self.id.load(Ordering::SeqCst) + 1;
        self.id.store(next_id, Ordering::SeqCst);

        let payload = Request::new(next_id, method, params);

        let body = serde_json::to_string(&payload).map_err(RelayError::RequestSerdeJson)?;

        let body_hash = keccak256(body.clone()).to_string();
        trace!("Body hash : {} {}", body_hash, body);

        let mut req = self.client.post(self.url.as_ref()).body(body).header("Content-Type", "application/json");

        if let Some(signer) = &self.signer {
            trace!("Signer on wallet  : {}", signer.address());
            let signature = signer.sign_message(body_hash.as_bytes()).await.map_err(RelayError::SignerError)?;

            req = req.header("X-Flashbots-Signature", format!("{}:0x{}", signer.address(), hex::encode(signature.as_bytes())));
        }

        let res = req.send().await?;
        let status = res.error_for_status_ref();

        match status {
            Err(err) => {
                let text = res.text().await?;
                let status_code = err.status().unwrap();
                if status_code.is_client_error() {
                    // Client error (400-499)
                    Err(RelayError::ClientError { text })
                } else {
                    // Internal server error (500-599)
                    Err(RelayError::RequestError(err))
                }
            }
            Ok(_) => {
                let text = res.text().await?;
                debug!("Flashbots response: {}", text);
                let res: Response<R> = serde_json::from_str(&text).map_err(|err| RelayError::ResponseSerdeJson { err, text })?;

                Ok(res.data.into_result()?)
            }
        }
    }

    pub async fn serialized_request<R: DeserializeOwned>(&self, body: String, signature: Option<String>) -> Result<R, RelayError> {
        let mut req = self.client.post(self.url.as_ref()).body(body).header("Content-Type", "application/json");

        if let Some(signature) = signature {
            req = req.header("X-Flashbots-Signature", signature);
        }

        let res = req.send().await?;
        let status = res.error_for_status_ref();

        match status {
            Err(err) => {
                let text = res.text().await?;
                let status_code = err.status().unwrap();
                if status_code.is_client_error() {
                    // Client error (400-499)
                    Err(RelayError::ClientError { text })
                } else {
                    // Internal server error (500-599)
                    Err(RelayError::RequestError(err))
                }
            }
            Ok(_) => {
                let text = res.text().await?;
                debug!("Flashbots response: {}", text);
                let res: Response<R> = serde_json::from_str(&text).map_err(|err| RelayError::ResponseSerdeJson { err, text })?;

                Ok(res.data.into_result()?)
            }
        }
    }
}
