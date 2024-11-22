use crate::{BackrunComposeData, RlpState, TxState};
use alloy_primitives::{BlockNumber, U256};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::{Swap, TxSigner};
use revm::DatabaseRef;

#[derive(Clone, Debug)]
pub enum TxBroadcastMessage<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    Sign(TxBroadcastData<LDT>),
    Broadcast(TxBroadcastData<LDT>),
}

#[derive(Clone, Debug)]
pub struct TxBroadcastData<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub eoa: Option<LDT::Address>,
    pub signer: Option<TxSigner>,
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
}

impl<LDT: LoomDataTypes> Default for TxBroadcastData<LDT> {
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
        }
    }
}
