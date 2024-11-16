use crate::evm::{revert_bytes_to_string, EvmError};
use alloy::primitives::map::HashSet;
use alloy::primitives::{Address, Bytes};
use alloy::rpc::types::trace::parity::{TraceType, TransactionTrace};
use eyre::eyre;
use revm::primitives::db::{Database, DatabaseCommit, DatabaseRef};
use revm::primitives::{Env, ExecutionResult, HaltReason, Output, TransactTo, CANCUN};
use revm::{inspector_handle_register, Evm};
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvmTraceError {
    #[error("Evm transact error")]
    TransactError,
    #[error("Evm transact commit error with err={0}")]
    TransactCommitError(String),
    #[error("Reverted with reason={0}, gas_used={1}")]
    Reverted(String, u64, Vec<TransactionTrace>),
    #[error("Halted with halt_reason={0:?}, gas_used={1}")]
    Halted(HaltReason, u64, Vec<TransactionTrace>),
}

fn parse_execution_result(
    execution_result: ExecutionResult,
    gas_used: u64,
    tx_trace: Vec<TransactionTrace>,
) -> eyre::Result<(Vec<u8>, u64, Vec<TransactionTrace>)> {
    match execution_result {
        ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used, tx_trace)),
        ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used, tx_trace)),
        ExecutionResult::Revert { output, gas_used } => {
            Err(eyre!(EvmTraceError::Reverted(revert_bytes_to_string(&output), gas_used, tx_trace)))
        }
        ExecutionResult::Halt { reason, gas_used } => Err(eyre!(EvmTraceError::Halted(reason, gas_used, tx_trace))),
    }
}

pub fn evm_trace_call<DB>(
    state_db: DB,
    env: Env,
    transact_to: Address,
    call_data_vec: Vec<u8>,
) -> eyre::Result<(Vec<u8>, u64, Vec<TransactionTrace>)>
where
    DB: DatabaseRef,
{
    let mut env = env;
    env.tx.transact_to = TransactTo::Call(transact_to);
    env.tx.data = Bytes::from(call_data_vec);

    let mut evm = Evm::builder()
        .with_ref_db(state_db)
        .with_spec_id(CANCUN)
        .with_env(Box::new(env))
        .with_external_context(TracingInspector::new(TracingInspectorConfig::from_parity_config(&HashSet::from_iter(vec![
            TraceType::Trace,
        ]))))
        .append_handler_register(inspector_handle_register)
        .build();

    let ref_tx = evm.transact().map_err(|_| EvmTraceError::TransactError)?;
    let execution_result = ref_tx.result;

    let gas_used = execution_result.gas_used();
    let tx_trace = evm.context.external.into_parity_builder().into_transaction_traces();

    parse_execution_result(execution_result, gas_used, tx_trace)
}

pub fn evm_trace_transact<DB>(evm: &mut Evm<TracingInspector, DB>) -> eyre::Result<(Vec<u8>, u64, Vec<TransactionTrace>)>
where
    DB: Database + DatabaseCommit,
{
    let execution_result = evm.transact_commit().map_err(|_| EvmError::TransactCommitError("COMMIT_ERROR".to_string()))?;
    let gas_used = execution_result.gas_used();
    let tx_trace = evm.context.external.clone().into_parity_builder().into_transaction_traces();

    parse_execution_result(execution_result, gas_used, tx_trace)
}
