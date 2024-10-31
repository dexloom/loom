use crate::client::jsonrpc::Request;
use crate::client::RelayError;
use alloy_primitives::{hex, keccak256};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use eyre::Result;
use serde::Serialize;

pub fn make_signed_body<R: Serialize + Send + Sync>(
    req_id: u64,
    method: &str,
    params: R,
    signer: &PrivateKeySigner,
) -> Result<(String, String)> {
    let payload = Request::new(req_id, method, [params]);

    let body = serde_json::to_string(&payload).map_err(RelayError::RequestSerdeJson)?;

    let body_hash = keccak256(body.clone()).to_string();

    let signature = signer.sign_message_sync(body_hash.as_bytes()).map_err(RelayError::SignerError)?;
    let fb_signature = format!("{}:0x{}", signer.address(), hex::encode(signature.as_bytes()));
    Ok((body, fb_signature))
}
