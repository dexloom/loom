use crate::dto::pool::PoolProtocol;
use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use utoipa::PartialSchema;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct Filter {
    pub protocol: Option<PoolProtocol>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct QuoteRequest {
    #[schema(schema_with = String::schema)]
    pub token_address_from: Address,
    #[schema(schema_with = String::schema)]
    pub token_address_to: Address,
    #[schema(schema_with = String::schema)]
    pub amount_in: U256,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct QuoteResponse {
    #[schema(schema_with = String::schema)]
    pub out_amount: U256,
    pub gas_used: u64,
}
