use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebSocketMessage {
    BlockHeader(BlockHeader),
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BlockHeader {
    pub number: u64,
    pub timestamp: u64,
    pub base_fee_per_gas: Option<u64>,
    pub next_block_base_fee: u64,
}
