use alloy_primitives::{Bytes, B256, U64};

use serde::{Deserialize, Serialize};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[derive(Debug, Deserialize)]
pub struct BundleRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[allow(dead_code)]
    pub id: u64,
    #[allow(dead_code)]
    pub method: String,
    pub params: Vec<BundleParam>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleParam {
    #[serde(rename = "txs")]
    pub transactions: Vec<Bytes>,

    #[allow(dead_code)]
    #[serde(rename = "blockNumber")]
    pub target_block: Option<U64>,
    // dropped the rest of the fields
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleResponse {
    pub bundle_hash: Option<B256>,
}

#[derive(Serialize)]
pub struct SendBundleResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: BundleResponse,
}

pub async fn mount_flashbots_mock(mock_server: &MockServer) {
    let bundle_resp = SendBundleResponse { jsonrpc: "2.0".to_string(), id: 1, result: BundleResponse { bundle_hash: Some(B256::ZERO) } };

    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&bundle_resp).append_header("content-type", "application/json"))
        .mount(mock_server)
        .await;
}
