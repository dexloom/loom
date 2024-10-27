use alloy_primitives::{Bytes, B256, U64};
use serde::{Deserialize, Serialize};
use utoipa::PartialSchema;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BundleParam {
    #[serde(rename = "txs")]
    #[schema(schema_with = String::schema)]
    pub transactions: Vec<Bytes>,

    #[serde(rename = "blockNumber")]
    #[schema(schema_with = String::schema)]
    pub target_block: Option<U64>,
    // dropped the rest of the fields
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BundleRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[allow(dead_code)]
    pub id: u64,
    #[allow(dead_code)]
    pub method: String,
    pub params: Vec<BundleParam>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BundleResponse {
    #[schema(schema_with = String::schema)]
    pub bundle_hash: Option<B256>,
}

#[derive(Serialize, ToSchema)]
pub struct SendBundleResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: BundleResponse,
}
