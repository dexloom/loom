use alloy_network::Network;
use alloy_primitives::{Address, BlockNumber, Bytes, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, BlockNumberOrTag, TransactionInput, TransactionRequest};
use alloy_rpc_types_trace::geth::AccountState;
use eyre::{eyre, Result};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::marker::PhantomData;
use tracing::{error, trace};

use crate::EntityAddress;
use loom_node_debug_provider::DebugProviderExt;
use loom_types_blockchain::{debug_trace_call_pre_state, GethStateUpdate, GethStateUpdateVec, LoomDataTypesEVM};
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum, LoomTransactionRequest};

#[derive(Clone, Debug, Default)]
pub struct RequiredState {
    calls: Vec<(EntityAddress, Bytes)>,
    slots: Vec<(EntityAddress, U256)>,
    empty_slots: Vec<(EntityAddress, U256)>,
}

impl RequiredState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_call<T: Into<Bytes>, A: Into<EntityAddress>>(&mut self, to: A, call_data: T) -> &mut Self {
        self.calls.push((to.into(), call_data.into()));
        self
    }
    pub fn add_slot<A: Into<EntityAddress>>(&mut self, address: A, slot: U256) -> &mut Self {
        self.slots.push((address.into(), slot));
        self
    }

    pub fn add_empty_slot<A: Into<EntityAddress>>(&mut self, address: A, slot: U256) -> &mut Self {
        self.empty_slots.push((address.into(), slot));
        self
    }

    pub fn add_empty_slot_range<A: Into<EntityAddress> + Clone>(&mut self, address: A, start_slot: U256, size: usize) -> &mut Self {
        let mut cur_slot = start_slot;
        for _ in 0..size {
            self.add_empty_slot(address.clone(), cur_slot);
            cur_slot += U256::from(1);
        }
        self
    }

    pub fn add_slot_range<A: Into<EntityAddress> + Clone>(&mut self, address: A, start_slot: U256, size: usize) -> &mut Self {
        let mut cur_slot = start_slot;
        for _ in 0..size {
            self.add_slot(address.clone(), cur_slot);
            cur_slot += U256::from(1);
        }
        self
    }
}

pub struct RequiredStateReader<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    _ldt: PhantomData<LDT>,
}

impl<LDT> RequiredStateReader<LDT>
where
    LDT: LoomDataTypesEVM,
{
    pub async fn fetch_calls_and_slots<
        N: Network<TransactionRequest = LDT::TransactionRequest>,
        C: DebugProviderExt<N> + Provider<N> + Clone + 'static,
    >(
        client: C,
        required_state: RequiredState,
        block_number: Option<BlockNumber>,
    ) -> Result<LDT::StateUpdate> {
        let block_id = if block_number.is_none() {
            BlockId::Number(BlockNumberOrTag::Latest)
        } else {
            BlockId::Number(BlockNumberOrTag::Number(block_number.unwrap_or_default()))
        };

        let mut ret: GethStateUpdate = GethStateUpdate::new();
        for req in required_state.calls.into_iter() {
            let to: LDT::Address = req.0.into();
            let req: LDT::TransactionRequest = LDT::TransactionRequest::build_call(to, req.1);

            let call_result = debug_trace_call_pre_state(client.clone(), req, block_id, None).await;
            trace!("trace_call_result: {:?}", call_result);
            match call_result {
                Ok(update) => {
                    for (address, account_state) in update.into_iter() {
                        let entry = ret.entry(address).or_insert(account_state.clone());
                        for (slot, value) in account_state.storage.clone().into_iter() {
                            entry.storage.insert(slot, value);
                            trace!(%address, %slot, %value, "Inserting storage");
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
            let value_result = client.get_storage_at(address.into(), slot).block_id(block_id).await;
            trace!("get_storage_at_result {} slot {} :  {:?}", address, slot, value_result);
            match value_result {
                Ok(value) => {
                    let entry = ret.entry(address.into()).or_default();
                    entry.storage.insert(slot.into(), value.into());
                }
                Err(e) => {
                    error!("{}", e)
                }
            }
        }

        for (address, slot) in required_state.empty_slots.into_iter() {
            let value = U256::ZERO;

            let entry = ret.entry(address.into()).or_default();
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

pub fn accounts_vec_len(state: &GethStateUpdateVec) -> usize {
    state.iter().map(|item| accounts_len(item).0).sum()
}

pub fn storage_vec_len(state: &GethStateUpdateVec) -> usize {
    state.iter().map(|item| accounts_len(item).1).sum()
}
