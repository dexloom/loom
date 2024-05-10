use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use alloy_primitives::{hex, keccak256};
use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use reqwest::{Client, Error as ReqwestError};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    client::bundle::BundleHash,
    client::jsonrpc::{JsonRpcError, Request, Response},
};

/// A Flashbots relay client.
///
/// The client automatically signs every request and sets the Flashbots
/// authorization header appropriately with the given signer.
///
/// **Note**: You probably do not want to use this directly, unless
/// you want to interact directly with the Relay. Most users should use
/// [`FlashbotsMiddleware`](crate::FlashbotsMiddleware) instead.
#[derive(Clone)]
pub struct Relay {
    id: Arc<AtomicU64>,
    client: Client,
    url: Url,
    signer: Option<LocalWallet>,
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
    ResponseSerdeJson {
        err: serde_json::Error,
        text: String,
    },
}

impl Relay {
    /// Initializes a new relay client.
    pub fn new(url: impl Into<Url>, signer: Option<LocalWallet>) -> Self {
        //let client = Client::builder().trust_dns(true).build().unwrap();
        let client = Client::new();

        Self {
            id: Arc::new(AtomicU64::new(0)),
            client,
            url: url.into(),
            signer,
        }
    }

    /// Sends a request with the provided method to the relay, with the
    /// parameters serialized as JSON.
    pub async fn request<T: Serialize + Send + Sync, R: DeserializeOwned>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, RelayError> {
        let next_id = self.id.load(Ordering::SeqCst) + 1;
        self.id.store(next_id, Ordering::SeqCst);

        let payload = Request::new(next_id, method, params);

        let mut req = self.client.post(self.url.as_ref());

        if let Some(signer) = &self.signer {
            let signature = signer
                .sign_message(
                    keccak256(
                        serde_json::to_string(&payload)
                            .map_err(RelayError::RequestSerdeJson)?
                            .as_bytes()
                    ).as_slice())
                .await
                .map_err(RelayError::SignerError)?;

            req = req.header(
                "X-Flashbots-Signature",
                format!("{:?}:0x{}", signer.address(), hex::encode(signature.as_bytes())),
            );
        }

        let res = req.json(&payload).send().await?;
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
                println!("Flashbots repsonse: {}", text);
                let res: Response<R> = serde_json::from_str(&text)
                    .map_err(|err| RelayError::ResponseSerdeJson { err, text })?;

                Ok(res.data.into_result()?)
            }
        }
    }
}


#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SendBundleResponse {
    #[serde(default)]
    pub(crate) bundle_hash: Option<BundleHash>,
}


