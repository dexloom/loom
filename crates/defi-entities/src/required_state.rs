use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::Range;
use std::slice::from_raw_parts_mut;
use std::sync::Arc;

use alloy_primitives::{Address, BlockNumber, Bytes, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_rpc_types_trace::geth::AccountState;
use eyre::{eyre, Result};
use log::{debug, error, trace};

use debug_provider::DebugProviderExt;
use defi_types::{debug_trace_call_pre_state, GethStateUpdate};

pub struct RequiredState {
    calls: Vec<TransactionRequest>,
    slots: Vec<(Address, U256)>,
    empty_slots: Vec<(Address, U256)>,
}

impl RequiredState {
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            slots: Vec::new(),
            empty_slots: Vec::new(),
        }
    }

    pub fn add_call<T: Into<Bytes> + Debug, A: Into<Address> + Debug>(&mut self, to: A, call_data: T) -> &mut Self {
        let req: TransactionRequest = TransactionRequest {
            gas: Some(1_000_000),
            to: Some(TxKind::Call(to.into())),
            input: TransactionInput::new(call_data.into()),
            ..TransactionRequest::default()
        };
        self.calls.push(req);
        self
    }
    pub fn add_slot(&mut self, address: Address, slot: U256) -> &mut Self {
        self.slots.push((address, slot));
        self
    }

    pub fn add_empty_slot(&mut self, address: Address, slot: U256) -> &mut Self {
        self.empty_slots.push((address, slot));
        self
    }


    pub fn add_empty_slot_range(&mut self, address: Address, start_slot: U256, size: usize) -> &mut Self {
        let mut cur_slot = start_slot;
        for _ in 0..size {
            self.add_empty_slot(address, cur_slot);
            cur_slot += U256::from(1);
        }
        self
    }

    pub fn add_slot_range(&mut self, address: Address, start_slot: U256, size: usize) -> &mut Self {
        let mut cur_slot = start_slot;
        for _ in 0..size {
            self.add_slot(address, cur_slot);
            cur_slot += U256::from(1);
        }
        self
    }
}

pub struct RequiredStateReader {}

impl RequiredStateReader {
    pub async fn fetch_calls_and_slots<C: DebugProviderExt + Provider + Clone + 'static>(client: C, required_state: RequiredState, block_number: Option<BlockNumber>) -> Result<GethStateUpdate> {
        let block_id = if block_number.is_none() { BlockNumberOrTag::Latest } else { BlockNumberOrTag::Number(block_number.unwrap_or_default()) };

        let mut ret: GethStateUpdate = GethStateUpdate::new();
        for req in required_state.calls.into_iter() {
            let to = req.to.unwrap_or_default().to().map_or(Address::ZERO, |x| *x);

            let call_result = debug_trace_call_pre_state(client.clone(), req, block_id, None).await;
            trace!("trace_call_result: {:?}", call_result);
            match call_result {
                Ok(update) => {
                    for (address, account_state) in update.into_iter() {
                        let mut entry = ret.entry(address).or_insert(account_state.clone());
                        for (slot, value) in account_state.storage.clone().into_iter() {
                            entry.storage.insert(slot, value);
                            //debug!("Inserting storage {:#20x} {} {}", address, slot, value);
                        }
                    }
                }
                Err(e) => {
                    error!("Contract call failed {} {}", to, e);
                    return Err(eyre!("CONTRACT_CALL_FAILED"));
                }
            }
        }
        for (address, slot) in required_state.slots.into_iter() {
            let value_result = client.get_storage_at(address, slot, BlockId::Number(block_id)).await;
            match value_result {
                Ok(value) => {
                    let mut entry = ret.entry(address).or_insert(AccountState::default());
                    entry.storage.insert(slot.into(), value.into());
                }
                Err(e) => { error!("{}",e) }
            }
        }

        for (address, slot) in required_state.empty_slots.into_iter() {
            let value = U256::ZERO;

            let mut entry = ret.entry(address).or_insert(AccountState::default());
            entry.storage.insert(slot.into(), value.into());
        }

        Ok(ret)
    }
}

pub fn accounts_len(state: &BTreeMap<Address, AccountState>) -> (usize, usize) {
    let accounts = state.len();
    let storage = state.values().map(|item| item.storage.clone().len()).sum();
    (accounts, storage)
}

pub fn accounts_vec_len(state: &Vec<BTreeMap<Address, AccountState>>) -> usize {
    state.iter().map(|item| accounts_len(item).1).sum()
}
