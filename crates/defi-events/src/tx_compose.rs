use std::ops::Deref;
use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_primitives::private::alloy_rlp;
use alloy_primitives::{Address, BlockNumber, Bytes, TxHash, U256};
use alloy_rlp::Encodable;
use alloy_rpc_types::{Transaction, TransactionRequest};
use eyre::{eyre, Result};

use defi_entities::{Swap, TxSigner};
use defi_types::GethStateUpdateVec;
use loom_revm_db::LoomDBType;

use crate::Message;

#[derive(Clone, Debug)]
pub enum TxState {
    Stuffing(Transaction),
    SignatureRequired(TransactionRequest),
    ReadyForBroadcast(Bytes),
    ReadyForBroadcastStuffing(Bytes),
}

impl TxState {
    pub fn rlp(&self) -> Result<Bytes> {
        match self {
            TxState::Stuffing(t) => {
                let mut r: Vec<u8> = Vec::new();
                let tenv: TxEnvelope = t.clone().try_into()?;
                tenv.encode(&mut r);
                Ok(Bytes::from(r))
            }
            TxState::ReadyForBroadcast(t) | TxState::ReadyForBroadcastStuffing(t) => Ok(t.clone()),
            _ => Err(eyre!("NOT_READY_FOR_BROADCAST")),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TxCompose {
    Route(TxComposeData),
    Estimate(TxComposeData),
    Sign(TxComposeData),
    Broadcast(TxComposeData),
}

impl Deref for TxCompose {
    type Target = TxComposeData;

    fn deref(&self) -> &Self::Target {
        self.data()
    }
}

impl TxCompose {
    pub fn data(&self) -> &TxComposeData {
        match self {
            TxCompose::Route(x) | TxCompose::Broadcast(x) | TxCompose::Sign(x) | TxCompose::Estimate(x) => x,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RlpState {
    Stuffing(Bytes),
    Backrun(Bytes),
    None,
}

impl RlpState {
    pub fn is_none(&self) -> bool {
        matches!(self, RlpState::None)
    }

    pub fn unwrap(&self) -> Bytes {
        match self.clone() {
            RlpState::Backrun(val) | RlpState::Stuffing(val) => val,
            RlpState::None => Bytes::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TxComposeData {
    /// The EOA address that will be used to sign the transaction.
    /// If this is None, the transaction will be signed by a random signer.
    pub eoa: Option<Address>,
    pub signer: Option<TxSigner>,
    pub nonce: u64,
    pub eth_balance: U256,
    pub value: U256,
    pub gas: u64,
    pub priority_gas_fee: u64,
    pub stuffing_txs_hashes: Vec<TxHash>,
    pub stuffing_txs: Vec<Transaction>,
    pub next_block_number: BlockNumber,
    pub next_block_timestamp: u64,
    pub next_block_base_fee: u64,
    pub swap: Swap,
    pub tx_bundle: Option<Vec<TxState>>,
    pub rlp_bundle: Option<Vec<RlpState>>,
    pub prestate: Option<Arc<LoomDBType>>,
    pub poststate: Option<Arc<LoomDBType>>,
    pub poststate_update: Option<GethStateUpdateVec>,
    pub origin: Option<String>,
    pub tips_pct: Option<u32>,
    pub tips: Option<U256>,
}

impl TxComposeData {
    pub fn same_stuffing(&self, others_stuffing_txs_hashes: &[TxHash]) -> bool {
        let tx_len = self.stuffing_txs_hashes.len();

        if tx_len != others_stuffing_txs_hashes.len() {
            false
        } else if tx_len == 0 {
            true
        } else {
            others_stuffing_txs_hashes.iter().all(|x| self.stuffing_txs_hashes.contains(x))
        }
    }

    pub fn cross_pools(&self, others_pools: &[Address]) -> bool {
        self.swap.get_pool_address_vec().iter().any(|x| others_pools.contains(x))
    }

    pub fn first_stuffing_hash(&self) -> TxHash {
        self.stuffing_txs_hashes.first().map_or(TxHash::default(), |x| *x)
    }

    pub fn tips_gas_ratio(&self) -> U256 {
        if self.gas == 0 {
            U256::ZERO
        } else {
            self.tips.unwrap_or_default() / U256::from(self.gas)
        }
    }

    pub fn profit_eth_gas_ratio(&self) -> U256 {
        if self.gas == 0 {
            U256::ZERO
        } else {
            self.swap.abs_profit_eth() / U256::from(self.gas)
        }
    }

    pub fn gas_price(&self) -> u128 {
        self.next_block_base_fee as u128 + self.priority_gas_fee as u128
    }

    pub fn gas_cost(&self) -> u128 {
        self.gas as u128 * (self.next_block_base_fee as u128 + self.priority_gas_fee as u128)
    }
}

impl Default for TxComposeData {
    fn default() -> Self {
        Self {
            eoa: None,
            signer: None,
            nonce: Default::default(),
            eth_balance: Default::default(),
            next_block_base_fee: Default::default(),
            value: Default::default(),
            gas: Default::default(),
            priority_gas_fee: Default::default(),
            stuffing_txs_hashes: Vec::new(),
            stuffing_txs: Vec::new(),
            next_block_number: Default::default(),
            next_block_timestamp: Default::default(),
            swap: Swap::None,
            tx_bundle: None,
            rlp_bundle: None,
            prestate: None,
            poststate: None,
            poststate_update: None,
            origin: None,
            tips_pct: None,
            tips: None,
        }
    }
}

pub type MessageTxCompose = Message<TxCompose>;

impl MessageTxCompose {
    pub fn route(data: TxComposeData) -> Self {
        Message::new(TxCompose::Route(data))
    }

    pub fn sign(data: TxComposeData) -> Self {
        Message::new(TxCompose::Sign(data))
    }

    pub fn estimate(data: TxComposeData) -> Self {
        Message::new(TxCompose::Estimate(data))
    }

    pub fn broadcast(data: TxComposeData) -> Self {
        Message::new(TxCompose::Broadcast(data))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let v = [RlpState::Stuffing(Bytes::from(vec![1])), RlpState::Backrun(Bytes::from(vec![2]))];

        let b: Vec<Bytes> = v.iter().filter(|i| matches!(i, RlpState::Backrun(_))).map(|i| i.unwrap()).collect();

        for c in b {
            println!("{c:?}");
        }
    }
}
