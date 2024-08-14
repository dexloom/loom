use std::fmt;

use crate::client::BundleHash;
use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
// Code adapted from: https://github.com/althea-net/guac_rs/tree/master/web3/src/jsonrpc
// NOTE: This module only exists since there is no way to use the data structures
// in the `ethers-providers/src/transports/common.rs` from another crate.

/// A JSON-RPC 2.0 error
#[derive(Serialize, Deserialize, Debug, Clone, Error)]
pub struct JsonRpcError {
    /// The error code
    pub code: i64,
    /// The error message
    pub message: String,
    /// Additional data
    pub data: Option<Value>,
}

impl fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(code: {}, message: {}, data: {:?})", self.code, self.message, self.data)
    }
}

fn is_zst<T>(_t: &T) -> bool {
    std::mem::size_of::<T>() == 0
}

#[derive(Serialize, Deserialize, Debug)]
/// A JSON-RPC request
pub struct Request<'a, T> {
    id: u64,
    jsonrpc: &'a str,
    method: &'a str,
    #[serde(skip_serializing_if = "is_zst")]
    params: T,
}

#[derive(Serialize, Deserialize, Debug)]
/// A JSON-RPC Notifcation
pub struct Notification<R> {
    jsonrpc: String,
    method: String,
    pub params: Subscription<R>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Subscription<R> {
    pub subscription: U256,
    pub result: R,
}

impl<'a, T> Request<'a, T> {
    /// Creates a new JSON RPC request
    pub fn new(id: u64, method: &'a str, params: T) -> Self {
        Self { id, jsonrpc: "2.0", method, params }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response<T> {
    #[serde(default)]
    pub(crate) id: u64,
    #[serde(default)]
    jsonrpc: String,
    #[serde(flatten)]
    pub data: ResponseData<T>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ResponseData<R> {
    Error { error: JsonRpcError },
    Success { result: R },
}

impl<R> ResponseData<R> {
    /// Consume response and return value
    pub fn into_result(self) -> Result<R, JsonRpcError> {
        match self {
            ResponseData::Success { result } => Ok(result),
            ResponseData::Error { error } => Err(error),
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum SendBundleResponseType {
    Integer(u64),
    BundleHash(BundleHash),
    String(String),
    SendBundleResponse(SendBundleResponse),
    Null(Option<()>),
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct SendBundleResponse {
    #[serde(default)]
    pub(crate) bundle_hash: Option<BundleHash>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{hex, TxHash};

    #[test]
    fn deser_response() {
        let response: Response<u64> = serde_json::from_str(r#"{"jsonrpc": "2.0", "result": 19, "id": 1}"#).unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(response.data.into_result().unwrap(), 19);
    }

    #[test]
    fn ser_request() {
        let request: Request<()> = Request::new(300, "method_name", ());
        assert_eq!(&serde_json::to_string(&request).unwrap(), r#"{"id":300,"jsonrpc":"2.0","method":"method_name"}"#);

        let request: Request<u32> = Request::new(300, "method_name", 1);
        assert_eq!(&serde_json::to_string(&request).unwrap(), r#"{"id":300,"jsonrpc":"2.0","method":"method_name","params":1}"#);
    }

    #[test]
    fn deser_response_enum() {
        let response: Response<SendBundleResponseType> = serde_json::from_str(r#"{"jsonrpc": "2.0", "result": 19, "id": 1}"#).unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(response.data.into_result().unwrap(), SendBundleResponseType::Integer(19));
    }

    #[test]
    fn deser_response_result_null() {
        // https://rpc.penguinbuild.org
        let response: Response<SendBundleResponseType> = serde_json::from_str(r#"{"jsonrpc":"2.0","id":2,"result":null}"#).unwrap();
        assert_eq!(response.id, 2);
        assert_eq!(response.data.into_result().unwrap(), SendBundleResponseType::Null(None));
    }

    #[test]
    fn deser_response_result_string() {
        // https://rpc.lokibuilder.xyz
        // https://api.securerpc.com/v1
        let response: Response<SendBundleResponseType> = serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":"nil"}"#).unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(response.data.into_result().unwrap(), SendBundleResponseType::String("nil".to_string()));
    }
    #[test]
    fn deser_response_result_send_bundle_response() {
        let response: Response<SendBundleResponseType> = serde_json::from_str(
            r#"{"id":1,"result":{"bundleHash":"0xcc6c61428c6516a252768859d167dc8f5c8c8c682334a184710f898e422530f8"},"jsonrpc":"2.0"}"#,
        )
        .unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(
            response.data.into_result().unwrap(),
            SendBundleResponseType::SendBundleResponse(SendBundleResponse {
                bundle_hash: Some(TxHash::from(hex!("cc6c61428c6516a252768859d167dc8f5c8c8c682334a184710f898e422530f8")))
            })
        )
    }
    #[test]
    fn deser_response_result_bundle_hash_response() {
        let response: Response<SendBundleResponseType> = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"result":"0xcc6c61428c6516a252768859d167dc8f5c8c8c682334a184710f898e422530f8"}"#,
        )
        .unwrap();
        assert_eq!(response.id, 1);
        assert_eq!(
            response.data.into_result().unwrap(),
            SendBundleResponseType::BundleHash(TxHash::from(hex!("cc6c61428c6516a252768859d167dc8f5c8c8c682334a184710f898e422530f8")))
        )
    }

    //
}
