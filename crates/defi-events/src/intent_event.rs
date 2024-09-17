use alloy_primitives::{Address, FixedBytes, U256};
use uniswapx_client_rs::types::order::{Order, OrderStatus, OrderType};

#[derive(Clone, Debug)]
pub enum IntentType {
    Dutch,
    Limit,
}

impl From<OrderType> for IntentType {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::Dutch => Self::Dutch,
            OrderType::DutchV2 => Self::Dutch,
            OrderType::Limit => Self::Limit,
            OrderType::DutchLimit => Self::Dutch,
        }
    }
}

#[derive(Clone, Debug)]
pub enum IntentStatus {
    Open,
    Expired,
    Error,
    Cancelled,
    Filled,
    InsufficientFunds,
}

impl From<OrderStatus> for IntentStatus {
    fn from(value: OrderStatus) -> Self {
        match value {
            OrderStatus::Open => Self::Open,
            OrderStatus::Expired => Self::Expired,
            OrderStatus::Error => Self::Error,
            OrderStatus::Cancelled => Self::Cancelled,
            OrderStatus::Filled => Self::Filled,
            OrderStatus::InsufficientFunds => Self::InsufficientFunds,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntentInput {
    pub token: Address,
    pub start_amount: U256,
    pub end_amount: U256,
}
#[derive(Debug, Clone)]
pub struct IntentOutput {
    pub token: Address,
    pub start_amount: U256,
    pub end_amount: U256,
    pub recipient: Address,
}

#[derive(Clone, Debug)]
pub struct IntentEvent {
    pub intent_type: IntentType,
    pub intent_status: IntentStatus,

    pub input: IntentInput,
    pub outputs: Vec<IntentOutput>,

    pub encoded_order: Vec<u8>,
    pub signature: FixedBytes<65>,
    pub nonce: U256,
    pub order_hash: FixedBytes<32>,
    pub chain_id: u8,

    pub created_at: u64,
    pub quote_id: Option<String>,
}

impl From<Order> for IntentEvent {
    fn from(order: Order) -> Self {
        Self {
            intent_type: order.order_type.into(),
            intent_status: order.order_status.into(),

            input: IntentInput { token: order.input.token, start_amount: order.input.start_amount, end_amount: order.input.end_amount },
            outputs: order
                .outputs
                .iter()
                .map(|output| IntentOutput {
                    token: output.token,
                    start_amount: output.start_amount,
                    end_amount: output.end_amount,
                    recipient: output.recipient,
                })
                .collect(),

            encoded_order: order.encoded_order,
            signature: order.signature,
            nonce: order.nonce,
            order_hash: order.order_hash,
            chain_id: order.chain_id,

            created_at: order.created_at,
            quote_id: order.quote_id,
        }
    }
}
