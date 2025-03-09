use crate::evm_env::{header_to_block_env, tx_req_to_env};
use alloy::consensus::BlockHeader;
use alloy::eips::BlockNumHash;
use alloy::primitives::TxHash;
use alloy::rpc::types::trace::geth::{AccountState, CallFrame};
use alloy::rpc::types::trace::parity::TransactionTrace;
use alloy::rpc::types::Log;
use alloy::{
    primitives::{Address, Bytes, B256, U256},
    rpc::types::{AccessList, AccessListItem, Header, Transaction, TransactionRequest},
};
use eyre::eyre;
use lazy_static::lazy_static;
use loom_evm_db::LoomDBError;
use loom_types_blockchain::GethStateUpdate;
use revm::context::result::{ExecutionResult, HaltReason, Output, ResultAndState};
use revm::context::setters::ContextSetters;
use revm::context::{Block, BlockEnv, CfgEnv, Evm, TransactTo, Transaction as EVMTransaction, TxEnv};
use revm::handler::EvmTr;
use revm::specification::hardfork::{CANCUN, LATEST};
use revm::state::Account;
use revm::{Context, Database, DatabaseCommit, DatabaseRef, JournaledState, MainBuilder, MainnetEvm};
use revm::{ExecuteCommitEvm, ExecuteEvm, MainContext};
use std::collections::BTreeMap;
use std::fmt::Debug;
use thiserror::Error;
use tracing::{debug, error, trace};

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

#[derive(Debug, Error)]
pub enum EvmError {
    #[error("Evm transact error")]
    TransactError,
    #[error("Evm transact commit error with err={0}")]
    TransactCommitError(String),
    #[error("Reverted with reason={0}, gas_used={1}")]
    Reverted(String, u64),
    #[error("Halted with halt_reason={0:?}, gas_used={1}")]
    Halted(HaltReason, u64),
}

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

#[derive(Debug, Error)]
pub enum EvmGethTraceError {
    #[error("Evm transact error")]
    TransactError,
    #[error("Evm transact commit error with err={0}")]
    TransactCommitError(String),
    #[error("Reverted with reason={0}, gas_used={1}")]
    Reverted(String, u64, CallFrame),
    #[error("Halted with halt_reason={0:?}, gas_used={1}")]
    Halted(HaltReason, u64, CallFrame),
}

pub struct EVMHelper<DB>(MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, DB, JournaledState<DB>>, ()>)
where
    DB: Database;

impl<DB> Clone for EVMHelper<DB>
where
    DB: Database + Clone,
    DB::Error: Clone,
{
    fn clone(&self) -> Self {
        EVMHelper(self.0.clone().build_mainnet())
    }
}

pub struct EVMParserHelper;

impl EVMParserHelper {
    pub fn parse_execution_result(execution_result: ExecutionResult, gas_used: u64) -> Result<(Vec<u8>, u64), EvmError> {
        match execution_result {
            ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used)),
            ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used)),
            ExecutionResult::Revert { output, gas_used } => Err(EvmError::Reverted(Self::revert_bytes_to_string(&output), gas_used)),
            ExecutionResult::Halt { reason, gas_used } => Err(EvmError::Halted(reason, gas_used)),
        }
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

    fn parse_trace_execution_result(
        execution_result: ExecutionResult,
        gas_used: u64,
        tx_trace: Vec<TransactionTrace>,
    ) -> eyre::Result<(Vec<u8>, u64, Vec<TransactionTrace>), EvmTraceError> {
        match execution_result {
            ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used, tx_trace)),
            ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used, tx_trace)),
            ExecutionResult::Revert { output, gas_used } => {
                Err(EvmTraceError::Reverted(Self::revert_bytes_to_string(&output), gas_used, tx_trace))
            }
            ExecutionResult::Halt { reason, gas_used } => Err(EvmTraceError::Halted(reason, gas_used, tx_trace)),
        }
    }

    fn parse_geth_trace_execution_result(
        execution_result: ExecutionResult,
        gas_used: u64,
        call_frame: CallFrame,
    ) -> eyre::Result<(Vec<u8>, u64, CallFrame), EvmGethTraceError> {
        match execution_result {
            ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used, call_frame)),
            ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used, call_frame)),
            ExecutionResult::Revert { output, gas_used } => {
                Err(EvmGethTraceError::Reverted(Self::revert_bytes_to_string(&output), gas_used, call_frame))
            }
            ExecutionResult::Halt { reason, gas_used } => Err(EvmGethTraceError::Halted(reason, gas_used, call_frame)),
        }
    }
}

impl<DB> EVMHelper<DB>
where
    DB: Database + DatabaseCommit,
{
    pub fn new<H: BlockHeader>(db: DB) -> Self {
        let ctx = Context::<BlockEnv, TxEnv, CfgEnv, DB, JournaledState<DB>>::new(db, LATEST);
        let evm = ctx.build_mainnet();
        EVMHelper(evm)
    }

    pub fn with_header<H: BlockHeader>(self, header: H) -> Self {
        let mut inner = self.0;
        let block_env = header_to_block_env(Some(header));
        inner.block = block_env;
        Self(inner)
    }

    pub fn with_beneficiary(self, coinbase: Option<Address>) -> Self {
        let mut inner = self.0;
        inner.modify_block(|x| x.beneficiary = coinbase.unwrap_or(*COINBASE));
        Self(inner)
    }

    pub fn evm_call<T>(&mut self, tx: T) -> Result<(Vec<u8>, u64), EvmError>
    where
        T: Into<TransactionRequest>,
    {
        let tx_env = tx_req_to_env(tx);

        let ref_tx = self.0.transact(tx_env).map_err(|_| EvmError::TransactError)?;
        let execution_result = ref_tx.result;

        let gas_used = execution_result.gas_used();

        EVMParserHelper::parse_execution_result(execution_result, gas_used)
    }

    pub fn evm_transact<T>(&mut self, tx: T) -> Result<(Vec<u8>, u64), EvmError>
    where
        T: Into<TransactionRequest>,
    {
        let tx_env = tx_req_to_env(tx);

        let execution_result = self.0.transact_commit(tx_env).map_err(|error| {
            error!(?error, "evm_transact evm.transact_commit()");
            EvmError::TransactCommitError("COMMIT_ERROR".to_string())
        })?;
        let gas_used = execution_result.gas_used();

        EVMParserHelper::parse_execution_result(execution_result, gas_used)
    }

    pub fn evm_access_list<T>(&mut self, tx: T) -> eyre::Result<(u64, AccessList)>
    where
        T: Into<TransactionRequest>,
    {
        let tx_env = tx_req_to_env(tx);

        let ref_tx = self.0.transact(tx_env).map_err(|_| EvmError::TransactError)?;
        let execution_result = ref_tx.result;
        match execution_result {
            ExecutionResult::Success { output, gas_used, reason, .. } => {
                trace!(gas_used, ?reason, ?output, "AccessList");
                let mut acl = AccessList::default();

                for (addr, acc) in ref_tx.state {
                    let storage_keys: Vec<B256> = acc.storage.keys().map(|x| (*x).into()).collect();
                    acl.0.push(AccessListItem { address: addr, storage_keys });
                }

                Ok((gas_used, acl))
            }
            ExecutionResult::Revert { output, gas_used } => {
                Err(eyre!(EvmError::Reverted(EVMParserHelper::revert_bytes_to_string(&output), gas_used)))
            }
            ExecutionResult::Halt { reason, gas_used } => Err(eyre!(EvmError::Halted(reason, gas_used))),
        }
    }
}

pub fn evm_call<E, T>(evm: &mut E, tx: T) -> eyre::Result<(Vec<u8>, u64), EvmError>
where
    E: ExecuteEvm<Output = Result<ExecutionResult, LoomDBError>> + ContextSetters<Tx = TxEnv>,
    T: Into<TransactionRequest>,
{
    let tx_env = tx_req_to_env(tx);

    let execution_result = evm.transact(tx_env).map_err(|_| EvmError::TransactError)?;

    let gas_used = execution_result.gas_used();

    EVMParserHelper::parse_execution_result(execution_result, gas_used)
}
