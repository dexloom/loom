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
use auto_impl::auto_impl;
use eyre::eyre;
use lazy_static::lazy_static;
use loom_evm_db::LoomDBError;
use loom_types_blockchain::GethStateUpdate;
use revm::bytecode::Bytecode;
use revm::context::result::{EVMError, ExecutionResult, HaltReason, Output, ResultAndState};
use revm::context::setters::ContextSetters;
use revm::context::{Block, BlockEnv, CfgEnv, ContextTr, DBErrorMarker, Evm, Journal, TransactTo, Transaction as EVMTransaction, TxEnv};
use revm::handler::EvmTr;
use revm::specification::hardfork::{CANCUN, LATEST};
use revm::state::{Account, AccountInfo};
use revm::{Context, Database, DatabaseCommit, DatabaseRef, JournaledState, MainBuilder, MainnetEvm};
use revm::{ExecuteCommitEvm, ExecuteEvm, MainContext};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};
use thiserror::Error;
use tracing::{debug, error, trace};

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

#[derive(Debug, Error)]
pub enum LoomEvmError {
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
pub enum LoomEvmTraceError {
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
pub enum LoomEvmGethTraceError {
    #[error("Evm transact error")]
    TransactError,
    #[error("Evm transact commit error with err={0}")]
    TransactCommitError(String),
    #[error("Reverted with reason={0}, gas_used={1}")]
    Reverted(String, u64, CallFrame),
    #[error("Halted with halt_reason={0:?}, gas_used={1}")]
    Halted(HaltReason, u64, CallFrame),
}

type LoomExecuteOutputType = Result<ResultAndState, EVMError<LoomDBError>>;

type LoomExecuteCommitOutputType = Result<ExecutionResult, EVMError<LoomDBError>>;

pub trait LoomExecuteCommitEvm:
    ExecuteCommitEvm<Output = LoomExecuteOutputType, CommitOutput = LoomExecuteCommitOutputType> + ContextSetters<Tx = TxEnv, Block = BlockEnv>
{
}
pub trait LoomExecuteEvm: ExecuteEvm<Output = LoomExecuteOutputType> + ContextSetters<Tx = TxEnv, Block = BlockEnv> {
    fn get_db_ref(&self) -> &dyn DatabaseRef<Error = LoomDBError>;
}

pub type LoomEVMType<DB: DatabaseRef<Error = LoomDBError> + Database<Error = LoomDBError> + DatabaseCommit> =
    MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, DB, JournaledState<DB>>, ()>;

impl<DB> LoomExecuteEvm for LoomEVMType<DB>
where
    DB: Database<Error = LoomDBError> + DatabaseRef<Error = LoomDBError> + DatabaseCommit + Clone,
{
    fn get_db_ref(&self) -> &dyn DatabaseRef<Error = LoomDBError> {
        let a = self.ctx_ref().db_ref();
        a as &dyn DatabaseRef<Error = LoomDBError>
    }
}

impl<DB> LoomExecuteCommitEvm for LoomEVMType<DB> where
    DB: Database<Error = LoomDBError> + DatabaseRef<Error = LoomDBError> + DatabaseCommit + Clone
{
}

pub struct LoomEVMWrapper<DB>(LoomEVMType<DB>)
where
    DB: Database;

impl<DB> Clone for LoomEVMWrapper<DB>
where
    DB: Database + Clone,
    DB::Error: Clone,
{
    fn clone(&self) -> Self {
        LoomEVMWrapper(self.0.clone().build_mainnet())
    }
}

impl<DB> LoomEVMWrapper<DB>
where
    DB: Database<Error = LoomDBError> + DatabaseRef<Error = LoomDBError> + DatabaseCommit + Clone + 'static,
{
    pub fn get_evm(&self) -> &LoomEVMType<DB> {
        &self.0
    }

    pub fn get_evm_mut(&mut self) -> &mut dyn LoomExecuteEvm {
        &mut self.0 as &mut dyn LoomExecuteEvm
    }

    pub fn get_mut(&mut self) -> &mut LoomEVMType<DB> {
        &mut self.0
    }
}

pub struct EVMParserHelper;

impl EVMParserHelper {
    pub fn parse_execution_result(execution_result: ExecutionResult, gas_used: u64) -> Result<(Vec<u8>, u64), LoomEvmError> {
        match execution_result {
            ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used)),
            ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used)),
            ExecutionResult::Revert { output, gas_used } => Err(LoomEvmError::Reverted(Self::revert_bytes_to_string(&output), gas_used)),
            ExecutionResult::Halt { reason, gas_used } => Err(LoomEvmError::Halted(reason, gas_used)),
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
    ) -> eyre::Result<(Vec<u8>, u64, Vec<TransactionTrace>), LoomEvmTraceError> {
        match execution_result {
            ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used, tx_trace)),
            ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used, tx_trace)),
            ExecutionResult::Revert { output, gas_used } => {
                Err(LoomEvmTraceError::Reverted(Self::revert_bytes_to_string(&output), gas_used, tx_trace))
            }
            ExecutionResult::Halt { reason, gas_used } => Err(LoomEvmTraceError::Halted(reason, gas_used, tx_trace)),
        }
    }

    fn parse_geth_trace_execution_result(
        execution_result: ExecutionResult,
        gas_used: u64,
        call_frame: CallFrame,
    ) -> eyre::Result<(Vec<u8>, u64, CallFrame), LoomEvmGethTraceError> {
        match execution_result {
            ExecutionResult::Success { output: Output::Call(value), .. } => Ok((value.to_vec(), gas_used, call_frame)),
            ExecutionResult::Success { output: Output::Create(_bytes, _address), .. } => Ok((vec![], gas_used, call_frame)),
            ExecutionResult::Revert { output, gas_used } => {
                Err(LoomEvmGethTraceError::Reverted(Self::revert_bytes_to_string(&output), gas_used, call_frame))
            }
            ExecutionResult::Halt { reason, gas_used } => Err(LoomEvmGethTraceError::Halted(reason, gas_used, call_frame)),
        }
    }
}

impl<DB> LoomEVMWrapper<DB>
where
    DB: Database<Error = loom_evm_db::LoomDBError> + DatabaseCommit + DatabaseRef<Error = loom_evm_db::LoomDBError>,
{
    pub fn new(db: DB) -> Self {
        let ctx = Context::<BlockEnv, TxEnv, CfgEnv, DB, JournaledState<DB>>::new(db, LATEST);
        let evm = ctx.build_mainnet();
        LoomEVMWrapper(evm)
    }

    pub fn with_header<H: BlockHeader>(self, header: &H) -> Self {
        let mut inner = self.0;
        let block_env = header_to_block_env(header);
        inner.block = block_env;
        Self(inner)
    }

    pub fn with_block_env(self, block_env: BlockEnv) -> Self {
        let mut inner = self.0;
        inner.block = block_env;
        Self(inner)
    }

    pub fn with_beneficiary(self, coinbase: Option<Address>) -> Self {
        let mut inner = self.0;
        inner.modify_block(|x| x.beneficiary = coinbase.unwrap_or(*COINBASE));
        Self(inner)
    }

    pub fn evm_call<T>(&mut self, tx: T) -> Result<(Vec<u8>, u64), LoomEvmError>
    where
        T: Into<TransactionRequest>,
    {
        evm_call(&mut self.0, tx)
    }

    pub fn evm_access_list<T>(&mut self, tx: T) -> Result<(u64, AccessList), LoomEvmError>
    where
        T: Into<TransactionRequest>,
    {
        evm_access_list(&mut self.0, tx)
    }

    pub fn db(&self) -> &DB {
        self.0.db_ref()
    }
}

pub fn evm_call<E, T>(evm: &mut E, tx: T) -> Result<(Vec<u8>, u64), LoomEvmError>
where
    E: ExecuteEvm<Output = Result<ResultAndState, EVMError<LoomDBError>>> + ContextSetters<Tx = TxEnv>,
    T: Into<TransactionRequest>,
{
    let tx_env = tx_req_to_env(tx);

    let result_and_state = evm.transact(tx_env).map_err(|_| LoomEvmError::TransactError)?;

    let execution_result = result_and_state.result;

    let gas_used = execution_result.gas_used();

    EVMParserHelper::parse_execution_result(execution_result, gas_used)
}

pub fn evm_call_raw<E, T>(evm: &mut E, tx: T) -> Result<ResultAndState, LoomEvmError>
where
    E: ExecuteEvm<Output = Result<ResultAndState, EVMError<LoomDBError>>> + ContextSetters<Tx = TxEnv>,
    T: Into<TransactionRequest>,
{
    let tx_env = tx_req_to_env(tx);

    evm.transact(tx_env).map_err(|_| LoomEvmError::TransactError)
}

pub fn evm_dyn_call<T>(evm: &mut dyn LoomExecuteEvm, tx: T) -> Result<(Vec<u8>, u64), LoomEvmError>
where
    T: Into<TransactionRequest>,
{
    let tx_env = tx_req_to_env(tx);

    let result_and_state = evm.transact(tx_env).map_err(|_| LoomEvmError::TransactError)?;

    let execution_result = result_and_state.result;

    let gas_used = execution_result.gas_used();

    EVMParserHelper::parse_execution_result(execution_result, gas_used)
}

fn evm_access_list<T, E>(evm: &mut E, tx: T) -> Result<(u64, AccessList), LoomEvmError>
where
    E: ExecuteEvm<Output = Result<ResultAndState, EVMError<LoomDBError>>> + ContextSetters<Tx = TxEnv>,
    T: Into<TransactionRequest>,
{
    let tx_env = tx_req_to_env(tx);

    let ref_tx = evm.transact(tx_env).map_err(|_| LoomEvmError::TransactError)?;
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
            Err(LoomEvmError::Reverted(EVMParserHelper::revert_bytes_to_string(&output), gas_used))
        }
        ExecutionResult::Halt { reason, gas_used } => Err(LoomEvmError::Halted(reason, gas_used)),
    }
}

pub fn evm_transact<T, E>(evm: &mut E, tx: T) -> Result<(Vec<u8>, u64), LoomEvmError>
where
    E: ExecuteCommitEvm<CommitOutput = Result<ResultAndState, EVMError<LoomDBError>>> + ContextSetters<Tx = TxEnv>,
    T: Into<TransactionRequest>,
{
    let tx_env = tx_req_to_env(tx);

    let result_and_state = evm.transact_commit(tx_env).map_err(|error| {
        error!(?error, "evm_transact evm.transact_commit()");
        LoomEvmError::TransactCommitError("COMMIT_ERROR".to_string())
    })?;
    let execution_result = result_and_state.result;

    let gas_used = execution_result.gas_used();

    EVMParserHelper::parse_execution_result(execution_result, gas_used)
}

pub fn evm_dyn_transact<T>(evm: &mut dyn LoomExecuteCommitEvm, tx: T) -> Result<(Vec<u8>, u64), LoomEvmError>
where
    T: Into<TransactionRequest>,
{
    let tx_env = tx_req_to_env(tx);

    let result_and_state = evm.transact_commit(tx_env).map_err(|error| {
        error!(?error, "evm_transact evm.transact_commit()");
        LoomEvmError::TransactCommitError("COMMIT_ERROR".to_string())
    })?;
    let execution_result = result_and_state;

    let gas_used = execution_result.gas_used();

    EVMParserHelper::parse_execution_result(execution_result, gas_used)
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::database::{CacheDB, EmptyDBTyped};

    #[test]
    fn test_evm_call() {
        let mut mainnet_evm = LoomEVMWrapper::new(CacheDB::new(EmptyDBTyped::<LoomDBError>::new()));
        evm_call(mainnet_evm.get_evm_mut(), TransactionRequest::default()).unwrap();
    }
}
