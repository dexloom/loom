use std::collections::BTreeMap;

use alloy_primitives::{Address, TxHash};
use revm::primitives::Env;
use revm::DatabaseRef;

use loom_evm_utils::evm_env::env_for_block;
use loom_types_blockchain::loom_data_types::{LoomDataTypes, LoomDataTypesEthereum};
use loom_types_entities::PoolWrapper;

#[derive(Clone)]
pub struct StateUpdateEvent<DB, LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub next_block_number: u64,
    pub next_block_timestamp: u64,
    pub next_base_fee: u64,
    market_state: DB,
    state_update: Vec<LDT::StateUpdate>,
    state_required: Option<Vec<LDT::StateUpdate>>,
    directions: BTreeMap<PoolWrapper, Vec<(Address, Address)>>,
    pub stuffing_txs_hashes: Vec<TxHash>,
    pub stuffing_txs: Vec<LDT::Transaction>,
    pub origin: String,
    pub tips_pct: u32,
}

#[allow(clippy::too_many_arguments)]
impl<DB: DatabaseRef, LDT: LoomDataTypes> StateUpdateEvent<DB, LDT> {
    pub fn new(
        next_block: u64,
        next_block_timestamp: u64,
        next_base_fee: u64,
        market_state: DB,
        state_update: Vec<LDT::StateUpdate>,
        state_required: Option<Vec<LDT::StateUpdate>>,
        directions: BTreeMap<PoolWrapper, Vec<(Address, Address)>>,
        stuffing_txs_hashes: Vec<TxHash>,
        stuffing_txs: Vec<LDT::Transaction>,
        origin: String,
        tips_pct: u32,
    ) -> StateUpdateEvent<DB, LDT> {
        StateUpdateEvent {
            next_block_number: next_block,
            next_block_timestamp,
            next_base_fee,
            state_update,
            state_required,
            market_state,
            directions,
            stuffing_txs_hashes,
            stuffing_txs,
            origin,
            tips_pct,
        }
    }

    pub fn evm_env(&self) -> Env {
        env_for_block(self.next_block_number, self.next_block_timestamp)
    }

    pub fn directions(&self) -> &BTreeMap<PoolWrapper, Vec<(Address, Address)>> {
        &self.directions
    }

    pub fn market_state(&self) -> &DB {
        &self.market_state
    }

    pub fn state_update(&self) -> &Vec<LDT::StateUpdate> {
        &self.state_update
    }

    pub fn state_required(&self) -> &Option<Vec<LDT::StateUpdate>> {
        &self.state_required
    }

    pub fn stuffing_len(&self) -> usize {
        self.stuffing_txs_hashes.len()
    }

    pub fn stuffing_txs_hashes(&self) -> &Vec<TxHash> {
        &self.stuffing_txs_hashes
    }
    pub fn stuffing_txs(&self) -> &Vec<LDT::Transaction> {
        &self.stuffing_txs
    }

    pub fn stuffing_tx_hash(&self) -> TxHash {
        self.stuffing_txs_hashes.first().cloned().unwrap_or_default()
    }
}
