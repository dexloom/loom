use crate::{Message, TxState};
use alloy_primitives::{BlockNumber, Bytes, U256};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{LoomTxSigner, Swap};
use std::sync::Arc;

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
pub enum TxComposeMessageType<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    Sign(TxComposeData<LDT>),
    Broadcast(TxComposeData<LDT>),
}

#[derive(Clone, Debug)]
pub struct TxComposeData<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    /// The EOA address that will be used to sign the transaction.
    /// If this is None, the transaction will be signed by a random signer.
    pub eoa: Option<LDT::Address>,
    pub signer: Option<Arc<dyn LoomTxSigner<LDT>>>,
    pub nonce: u64,
    pub eth_balance: U256,
    pub value: U256,
    pub gas: u64,
    pub priority_gas_fee: u64,
    pub stuffing_txs_hashes: Vec<LDT::TxHash>,
    pub stuffing_txs: Vec<LDT::Transaction>,
    pub next_block_number: BlockNumber,
    pub next_block_timestamp: u64,
    pub next_block_base_fee: u64,
    pub tx_bundle: Option<Vec<TxState<LDT>>>,
    pub rlp_bundle: Option<Vec<RlpState>>,
    pub origin: Option<String>,
    pub swap: Option<Swap>,
    pub tips: Option<U256>,
}

impl<LDT: LoomDataTypes> Default for TxComposeData<LDT> {
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
            tx_bundle: None,
            rlp_bundle: None,
            origin: None,
            swap: None,
            tips: None,
        }
    }
}

pub type MessageTxCompose<LDT = LoomDataTypesEthereum> = Message<TxComposeMessageType<LDT>>;

impl<LDT: LoomDataTypes> MessageTxCompose<LDT> {
    pub fn sign(data: TxComposeData<LDT>) -> Self {
        Message::new(TxComposeMessageType::Sign(data))
    }

    pub fn broadcast(data: TxComposeData<LDT>) -> Self {
        Message::new(TxComposeMessageType::Broadcast(data))
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
