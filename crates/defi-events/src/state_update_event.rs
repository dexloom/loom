use std::collections::BTreeMap;
use std::fmt::Debug;

use alloy_primitives::{Address, TxHash};
use alloy_rpc_types::Transaction;
use revm::primitives::Env;

use defi_entities::PoolWrapper;
use defi_types::GethStateUpdateVec;
use loom_revm_db::LoomInMemoryDB;
use loom_utils::evm::env_for_block;

#[derive(Clone, Debug)]
pub struct StateUpdateEvent
{
    pub block: u64,
    pub block_timestamp: u64,
    pub gas_fee: u128,
    market_state: LoomInMemoryDB,
    state_update: GethStateUpdateVec,
    state_required: Option<GethStateUpdateVec>,
    directions: BTreeMap<PoolWrapper, Vec<(Address, Address)>>,
    pub stuffing_txs_hashes: Vec<TxHash>,
    pub stuffing_txs: Vec<Transaction>,
    pub origin: String,
    pub tips_pct: u32,

}

impl StateUpdateEvent
{
    pub fn new(
        block: u64,
        block_timestamp: u64,
        gas_fee: u128,
        market_state: LoomInMemoryDB,
        state_update: GethStateUpdateVec,
        state_required: Option<GethStateUpdateVec>,
        directions: BTreeMap<PoolWrapper, Vec<(Address, Address)>>,
        stuffing_txs_hashes: Vec<TxHash>,
        stuffing_txs: Vec<Transaction>,
        origin: String,
        tips_pct: u32,
    ) -> StateUpdateEvent {
        StateUpdateEvent {
            block,
            block_timestamp,
            gas_fee,
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
        env_for_block(self.block, self.block_timestamp)
    }


    pub fn directions(&self) -> &BTreeMap<PoolWrapper, Vec<(Address, Address)>> {
        &self.directions
    }

    pub fn market_state(&self) -> &LoomInMemoryDB {
        &self.market_state
    }

    pub fn state_update(&self) -> &GethStateUpdateVec {
        &self.state_update
    }

    pub fn state_required(&self) -> &Option<GethStateUpdateVec> {
        &self.state_required
    }


    pub fn stuffing_len(&self) -> usize {
        self.stuffing_txs_hashes.len()
    }

    pub fn stuffing_txs_hashes(&self) -> &Vec<TxHash> {
        &self.stuffing_txs_hashes
    }
    pub fn stuffing_txs(&self) -> &Vec<Transaction> {
        &self.stuffing_txs
    }

    pub fn stuffing_tx_hash(&self) -> TxHash {
        self.stuffing_txs_hashes.first().cloned().unwrap_or_default()
    }
}
