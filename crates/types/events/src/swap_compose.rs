use crate::tx_compose::TxComposeData;
use crate::Message;
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Bytes, U256};
use eyre::{eyre, Result};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{PoolId, Swap};
use revm::DatabaseRef;
use std::ops::Deref;

#[derive(Clone, Debug)]
pub enum TxState<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    Stuffing(LDT::Transaction),
    SignatureRequired(LDT::TransactionRequest),
    ReadyForBroadcast(Bytes),
    ReadyForBroadcastStuffing(Bytes),
}

impl TxState {
    pub fn rlp(&self) -> Result<Bytes> {
        match self {
            TxState::Stuffing(t) => Ok(Bytes::from(t.clone().inner.encoded_2718())),
            TxState::ReadyForBroadcast(t) | TxState::ReadyForBroadcastStuffing(t) => Ok(t.clone()),
            _ => Err(eyre!("NOT_READY_FOR_BROADCAST")),
        }
    }
}

#[derive(Clone, Debug)]
pub enum SwapComposeMessage<DB, LDT: LoomDataTypes = LoomDataTypesEthereum> {
    Prepare(SwapComposeData<DB, LDT>),
    Estimate(SwapComposeData<DB, LDT>),
    Ready(SwapComposeData<DB, LDT>),
}

impl<DB, LDT: LoomDataTypes> Deref for SwapComposeMessage<DB, LDT> {
    type Target = SwapComposeData<DB, LDT>;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl<DB, LDT: LoomDataTypes> SwapComposeMessage<DB, LDT> {
    pub fn data(&self) -> &SwapComposeData<DB, LDT> {
        match self {
            SwapComposeMessage::Prepare(x) | SwapComposeMessage::Estimate(x) | SwapComposeMessage::Ready(x) => x,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SwapComposeData<DB, LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub tx_compose: TxComposeData<LDT>,
    pub swap: Swap<LDT>,
    pub prestate: Option<DB>,
    pub poststate: Option<DB>,
    pub poststate_update: Option<Vec<LDT::StateUpdate>>,
    pub origin: Option<String>,
    pub tips_pct: Option<u32>,
    pub tips: Option<U256>,
}

impl<DB: Clone + 'static, LDT: LoomDataTypes> SwapComposeData<DB, LDT> {
    pub fn same_stuffing(&self, others_stuffing_txs_hashes: &[LDT::TxHash]) -> bool {
        let tx_len = self.tx_compose.stuffing_txs_hashes.len();

        if tx_len != others_stuffing_txs_hashes.len() {
            false
        } else if tx_len == 0 {
            true
        } else {
            others_stuffing_txs_hashes.iter().all(|x| self.tx_compose.stuffing_txs_hashes.contains(x))
        }
    }

    pub fn cross_pools(&self, others_pools: &[PoolId<LDT>]) -> bool {
        self.swap.get_pool_id_vec().iter().any(|x| others_pools.contains(x))
    }

    pub fn first_stuffing_hash(&self) -> LDT::TxHash {
        self.tx_compose.stuffing_txs_hashes.first().map_or(LDT::TxHash::default(), |x| *x)
    }

    pub fn tips_gas_ratio(&self) -> U256 {
        if self.tx_compose.gas == 0 {
            U256::ZERO
        } else {
            self.tips.unwrap_or_default() / U256::from(self.tx_compose.gas)
        }
    }

    pub fn profit_eth_gas_ratio(&self) -> U256 {
        if self.tx_compose.gas == 0 {
            U256::ZERO
        } else {
            self.swap.abs_profit_eth() / U256::from(self.tx_compose.gas)
        }
    }

    pub fn gas_price(&self) -> u128 {
        self.tx_compose.next_block_base_fee as u128 + self.tx_compose.priority_gas_fee as u128
    }

    pub fn gas_cost(&self) -> u128 {
        self.tx_compose.gas as u128 * (self.tx_compose.next_block_base_fee as u128 + self.tx_compose.priority_gas_fee as u128)
    }
}

impl<DB: DatabaseRef + Send + Sync + Clone + 'static, LDT: LoomDataTypes> Default for SwapComposeData<DB, LDT> {
    fn default() -> Self {
        Self {
            tx_compose: Default::default(),
            swap: Swap::None,
            prestate: None,
            poststate: None,
            poststate_update: None,
            origin: None,
            tips_pct: None,
            tips: None,
        }
    }
}

pub type MessageSwapCompose<DB, LDT = LoomDataTypesEthereum> = Message<SwapComposeMessage<DB, LDT>>;

impl<DB, LDT: LoomDataTypes> MessageSwapCompose<DB, LDT> {
    pub fn prepare(data: SwapComposeData<DB, LDT>) -> Self {
        Message::new(SwapComposeMessage::Prepare(data))
    }

    pub fn estimate(data: SwapComposeData<DB, LDT>) -> Self {
        Message::new(SwapComposeMessage::Estimate(data))
    }

    pub fn ready(data: SwapComposeData<DB, LDT>) -> Self {
        Message::new(SwapComposeMessage::Ready(data))
    }
}
