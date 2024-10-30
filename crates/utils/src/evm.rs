use crate::evm_env::evm_env_from_tx;
use alloy::eips::BlockNumHash;
use alloy::primitives::TxHash;
use alloy::rpc::types::trace::geth::AccountState;
#[cfg(feature = "trace-calls")]
use alloy::rpc::types::trace::parity::TraceType;
use alloy::rpc::types::Log;
use alloy::{
    primitives::{Address, Bytes, B256, U256},
    rpc::types::{AccessList, AccessListItem, Header, Transaction, TransactionRequest},
};
use defi_types::GethStateUpdate;
use eyre::eyre;
use lazy_static::lazy_static;
#[cfg(feature = "trace-calls")]
use revm::inspector_handle_register;
#[cfg(feature = "trace-calls")]
use revm::primitives::HashSet;
use revm::primitives::{Account, Env, ExecutionResult, HaltReason, Output, ResultAndState, TransactTo, SHANGHAI};
use revm::{Database, DatabaseCommit, DatabaseRef, Evm};
#[cfg(feature = "trace-calls")]
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use std::collections::BTreeMap;
use std::fmt::Display;
use thiserror::Error;
use tracing::{debug, error};

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

#[derive(Debug, Error)]
pub enum EvmError {
    #[error("Evm transact error")]
    TransactError,
    #[error("Evm transact commit error with err={0}")]
    TransactCommitError(String),
    #[error("Reverted with reason={0}")]
    Reverted(String, u64),
    #[error("Halted with halt_reason={0:?}")]
    Halted(HaltReason, u64),
}

pub fn evm_call<DB>(state_db: DB, env: Env, transact_to: Address, call_data_vec: Vec<u8>) -> eyre::Result<(Vec<u8>, u64)>
where
    DB: DatabaseRef,
{
    let mut env = env;
    env.tx.transact_to = TransactTo::Call(transact_to);
    env.tx.data = Bytes::from(call_data_vec);

    #[cfg(feature = "trace-calls")]
    let mut evm = Evm::builder()
        .with_ref_db(state_db)
        .with_spec_id(SHANGHAI)
        .with_env(Box::new(env))
        .with_external_context(TracingInspector::new(TracingInspectorConfig::from_parity_config(&HashSet::from_iter(vec![
            TraceType::Trace,
        ]))))
        .append_handler_register(inspector_handle_register)
        .build();

    #[cfg(not(feature = "trace-calls"))]
    let mut evm = Evm::builder().with_spec_id(SHANGHAI).with_ref_db(state_db).with_env(Box::new(env)).build();

    let ref_tx = evm.transact().map_err(|_| EvmError::TransactError)?;
    let execution_result = ref_tx.result;

    let gas_used = execution_result.gas_used();

    match execution_result {
        ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used)),
        ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used)),
        ExecutionResult::Revert { output, gas_used } => {
            #[cfg(feature = "trace-calls")]
            debug!("Revert trace: {:#?}", evm.context.external.into_parity_builder().into_transaction_traces());

            Err(eyre!(EvmError::Reverted(revert_bytes_to_string(&output), gas_used)))
        }
        ExecutionResult::Halt { reason, gas_used } => Err(eyre!(EvmError::Halted(reason, gas_used))),
    }
}

pub fn evm_transact<DB>(evm: &mut Evm<(), DB>) -> eyre::Result<(Vec<u8>, u64)>
where
    DB: Database + DatabaseCommit,
    <DB as Database>::Error: Display,
{
    let execution_result = evm.transact_commit().map_err(|e| EvmError::TransactCommitError(e.to_string()))?;
    let gas_used = execution_result.gas_used();

    match execution_result {
        ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used)),
        ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used)),
        ExecutionResult::Revert { output, gas_used } => Err(eyre!(EvmError::Reverted(revert_bytes_to_string(&output), gas_used))),
        ExecutionResult::Halt { reason, gas_used } => Err(eyre!(EvmError::Halted(reason, gas_used))),
    }
}

pub fn evm_access_list<DB: DatabaseRef>(state_db: DB, env: &Env, tx: &TransactionRequest) -> eyre::Result<(u64, AccessList)>
where
    <DB as DatabaseRef>::Error: Display,
{
    let mut env = env.clone();

    let txto = tx.to.unwrap_or_default().to().map_or(Address::ZERO, |x| *x);

    env.tx.chain_id = tx.chain_id;
    env.tx.transact_to = TransactTo::Call(txto);
    env.tx.nonce = tx.nonce;
    env.tx.data = tx.input.clone().input.unwrap();
    env.tx.value = tx.value.unwrap_or_default();
    env.tx.caller = tx.from.unwrap_or_default();
    env.tx.gas_price = U256::from(tx.max_fee_per_gas.unwrap_or(tx.gas_price.unwrap_or_default()));
    env.tx.gas_limit = tx.gas.unwrap_or_default();
    env.tx.gas_priority_fee = Some(U256::from(tx.max_priority_fee_per_gas.unwrap_or_default()));

    env.block.coinbase = *COINBASE;

    #[cfg(feature = "trace-calls")]
    let mut evm = Evm::builder()
        .with_ref_db(state_db)
        .with_spec_id(SHANGHAI)
        .with_env(Box::new(env))
        .with_external_context(TracingInspector::new(TracingInspectorConfig::from_parity_config(&HashSet::from_iter(vec![
            TraceType::Trace,
        ]))))
        .append_handler_register(inspector_handle_register)
        .build();

    #[cfg(not(feature = "trace-calls"))]
    let mut evm = Evm::builder().with_ref_db(state_db).with_spec_id(SHANGHAI).with_env(Box::new(env)).build();

    let ref_tx = evm.transact().map_err(|_| EvmError::TransactError)?;
    let execution_result = ref_tx.result;
    match execution_result {
        ExecutionResult::Success { output, gas_used, reason, .. } => {
            debug!(gas_used, ?reason, ?output, "AccessList");
            let mut acl = AccessList::default();

            for (addr, acc) in ref_tx.state {
                let storage_keys: Vec<B256> = acc.storage.keys().map(|x| (*x).into()).collect();
                acl.0.push(AccessListItem { address: addr, storage_keys });
            }

            Ok((gas_used, acl))
        }
        ExecutionResult::Revert { output, gas_used } => {
            #[cfg(feature = "trace-calls")]
            debug!("Trace: {:#?}", evm.context.external.into_parity_builder().into_transaction_traces());

            Err(eyre!(EvmError::Reverted(revert_bytes_to_string(&output), gas_used)))
        }
        ExecutionResult::Halt { reason, gas_used } => Err(eyre!(EvmError::Halted(reason, gas_used))),
    }
}

pub fn evm_call_tx_in_block<DB, T: Into<Transaction>>(tx: T, state_db: DB, header: &Header) -> eyre::Result<ResultAndState>
where
    DB: DatabaseRef,
    <DB as DatabaseRef>::Error: Display,
{
    let env = evm_env_from_tx(tx, header);

    let mut evm = Evm::builder().with_spec_id(SHANGHAI).with_ref_db(state_db).with_env(Box::new(env)).build();

    evm.transact().map_err(|e| {
        error!("evm.transact : {e}");
        eyre!("TRANSACT_ERROR")
    })
}

pub fn convert_evm_result_to_rpc(
    result: ResultAndState,
    tx_hash: TxHash,
    block_num_hash: BlockNumHash,
    block_timestamp: u64,
) -> eyre::Result<(Vec<Log>, GethStateUpdate)> {
    let logs = match result.result {
        ExecutionResult::Success { logs, .. } => logs
            .into_iter()
            .enumerate()
            .map(|(log_index, l)| Log {
                inner: l.clone(),
                block_hash: Some(block_num_hash.hash),
                block_number: Some(block_num_hash.number),
                transaction_hash: Some(tx_hash),
                transaction_index: Some(0u64),
                log_index: Some(log_index as u64),
                removed: false,
                block_timestamp: Some(block_timestamp),
            })
            .collect(),
        _ => return Err(eyre!("EXECUTION_REVERTED")),
    };

    let mut state_update: GethStateUpdate = GethStateUpdate::default();

    for (address, account) in result.state.into_iter() {
        let (address, account): (Address, Account) = (address, account);
        let storage: BTreeMap<B256, B256> = account.storage.into_iter().map(|(k, v)| (k.into(), v.present_value.into())).collect();

        let account_state = AccountState {
            balance: Some(account.info.balance),
            code: account.info.code.map(|x| x.bytes()),
            nonce: Some(account.info.nonce),
            storage,
        };
        state_update.insert(address, account_state);
    }

    Ok((logs, state_update))
}

pub fn revert_bytes_to_string(bytes: &Bytes) -> String {
    if bytes.len() < 4 {
        return format!("{:?}", bytes);
    }
    let error_data = &bytes[4..];

    match String::from_utf8(error_data.to_vec()) {
        Ok(s) => s.replace(char::from(0), "").trim().to_string(),
        Err(_) => format!("{:?}", bytes),
    }
}
