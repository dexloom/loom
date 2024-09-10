use std::collections::BTreeMap;
use std::convert::Infallible;

use alloy::eips::BlockNumHash;
use alloy::primitives::TxHash;
use alloy::rpc::types::trace::geth::AccountState;
use alloy::rpc::types::Log;
use alloy::{
    primitives::{Address, Bytes, B256, U256},
    rpc::types::{AccessList, AccessListItem, Header, Transaction, TransactionRequest},
};
use defi_types::GethStateUpdate;
use eyre::{eyre, OptionExt, Result};
use lazy_static::lazy_static;
use log::{debug, error, trace};
use revm::interpreter::Host;
use revm::primitives::{Account, BlockEnv, Env, ExecutionResult, Output, ResultAndState, TransactTo, TxEnv, SHANGHAI};
use revm::{Database, DatabaseCommit, DatabaseRef, Evm};

pub fn env_for_block(block_id: u64, block_timestamp: u64) -> Env {
    let mut env = Env::default();
    env.block.timestamp = U256::from(block_timestamp);
    env.block.number = U256::from(block_id);
    env
}

pub fn evm_call<DB>(state_db: DB, env: Env, transact_to: Address, call_data_vec: Vec<u8>) -> Result<(Vec<u8>, u64)>
where
    DB: DatabaseRef<Error = Infallible>,
{
    let mut env = env;
    env.tx.transact_to = TransactTo::Call(transact_to);
    env.tx.data = Bytes::from(call_data_vec);
    env.tx.value = U256::from(0);

    let mut evm = Evm::builder().with_spec_id(SHANGHAI).with_ref_db(state_db).with_env(Box::new(env)).build();

    let ref_tx = evm.transact().unwrap();
    let result = ref_tx.result;

    let gas_used = result.gas_used();

    // unpack output call enum into raw bytes
    let value = match result {
        ExecutionResult::Success { output: Output::Call(value), .. } => Some((value.to_vec(), gas_used)),
        ExecutionResult::Revert { output, gas_used } => {
            trace!("Revert {} : {:?}", gas_used, output);
            None
        }
        _ => None,
    };

    value.ok_or_eyre("CALL_RESULT_IS_EMPTY")
}

pub fn evm_transact<DB>(evm: &mut Evm<(), DB>, tx: &Transaction) -> Result<()>
where
    DB: Database<Error = Infallible> + DatabaseCommit,
{
    let env = evm.context.env_mut();

    env.tx.transact_to = TransactTo::Call(tx.to.unwrap());
    env.tx.nonce = Some(tx.nonce);
    env.tx.data = tx.input.clone();
    env.tx.value = tx.value;
    env.tx.caller = tx.from;
    env.tx.gas_price = U256::from(tx.max_fee_per_gas.unwrap_or(tx.gas_price.unwrap_or_default()));
    env.tx.gas_limit = tx.gas as u64;
    env.tx.gas_priority_fee = Some(U256::from(tx.max_priority_fee_per_gas.unwrap_or_default()));

    match evm.transact_commit() {
        Ok(execution_result) => match execution_result {
            ExecutionResult::Success { output, gas_used, reason, .. } => {
                debug!("Transact Gas used : {gas_used} reason:  {reason:?}");
                debug!("Transact Output : {output:?}");

                Ok(())
            }
            ExecutionResult::Revert { output, gas_used } => {
                error!("Revert {output} Gas used {gas_used}");
                Err(eyre!("EXECUTION_REVERTED"))
            }
            ExecutionResult::Halt { reason, .. } => {
                error!("Halt {reason:?}");
                Err(eyre!("EXECUTION_HALT"))
            }
        },
        Err(e) => {
            error!("Execution error : {e}");
            Err(eyre!("EXECUTION_ERROR"))
        }
    }
}

lazy_static! {
    static ref COINBASE: Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

pub fn evm_access_list<DB>(state_db: DB, env: &Env, tx: &TransactionRequest) -> Result<(u64, AccessList)>
where
    DB: DatabaseRef<Error = Infallible>,
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
    env.tx.gas_limit = tx.gas.unwrap_or_default() as u64;
    env.tx.gas_priority_fee = Some(U256::from(tx.max_priority_fee_per_gas.unwrap_or_default()));

    env.block.coinbase = *COINBASE;

    let mut evm = Evm::builder().with_ref_db(state_db).with_spec_id(SHANGHAI).with_env(Box::new(env)).build();

    match evm.transact() {
        Ok(execution_result) => match execution_result.result {
            ExecutionResult::Success { output, gas_used, reason, .. } => {
                debug!("AccessList Gas used : {gas_used} reason : {reason:?}");
                debug!("AccessList Output : {output:?}");
                let mut acl = AccessList::default();

                for (addr, acc) in execution_result.state {
                    let storage_keys: Vec<B256> = acc.storage.keys().map(|x| (*x).into()).collect();
                    acl.0.push(AccessListItem { address: addr, storage_keys });
                }

                Ok((gas_used, acl))
            }
            ExecutionResult::Revert { output, gas_used } => {
                error!("Revert {output} gas used {gas_used}");
                Err(eyre!("EXECUTION_REVERTED"))
            }
            ExecutionResult::Halt { reason, .. } => {
                error!("Halt {reason:?}");
                Err(eyre!("EXECUTION_HALT"))
            }
        },
        Err(e) => {
            error!("Execution error : {e}");
            Err(eyre!("EXECUTION_ERROR"))
        }
    }
}

pub fn evm_env_from_tx<T: Into<Transaction>>(tx: T, block_header: &Header) -> Env {
    let tx = tx.into();

    /*let blob_gas = if block_header.blob_gas_used.is_some() && block_header.excess_blob_gas.is_some() {
        Some(BlobExcessGasAndPrice {
            excess_blob_gas: block_header.blob_gas_used.unwrap_or_default() as u64,
            blob_gasprice: block_header.excess_blob_gas.unwrap_or_default(),
        })
    } else {
        None
    };

     */

    Env {
        cfg: Default::default(),
        block: BlockEnv {
            number: U256::from(block_header.number),
            coinbase: block_header.miner,
            timestamp: U256::from(block_header.timestamp),
            gas_limit: U256::from(block_header.gas_limit),
            basefee: U256::from(block_header.base_fee_per_gas.unwrap_or_default()),
            difficulty: block_header.difficulty,
            prevrandao: Some(block_header.parent_hash),
            blob_excess_gas_and_price: None,
        },
        tx: TxEnv {
            caller: tx.from,
            gas_limit: tx.gas as u64,
            gas_price: U256::from(tx.gas_price.unwrap_or_default()),
            transact_to: TransactTo::Call(tx.to.unwrap_or_default()),
            value: tx.value,
            data: tx.input,
            nonce: Some(tx.nonce),
            chain_id: tx.chain_id,
            access_list: Vec::new(),
            gas_priority_fee: tx.max_priority_fee_per_gas.map(|x| U256::from(x)),
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: None,
            authorization_list: None,
            //eof_initcodes: vec![],
            //eof_initcodes_hashed: Default::default(),
        },
    }
}

pub fn evm_call_tx_in_block<DB, T: Into<Transaction>>(tx: T, state_db: DB, header: &Header) -> Result<ResultAndState>
where
    DB: DatabaseRef<Error = Infallible>,
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
) -> Result<(Vec<Log>, GethStateUpdate)> {
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
