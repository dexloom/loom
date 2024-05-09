use alloy_primitives::{Address, B256, Bytes, U256};
use alloy_rpc_types::{AccessList, AccessListItem, Header, Transaction, TransactionRequest};
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use log::{debug, error, trace};
use revm::{Context, EvmContext, Handler, InMemoryDB};
use revm::db::WrapDatabaseRef;
use revm::Evm;
use revm::interpreter::Host;
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, Env, ExecutionResult, Output, SHANGHAI, ShanghaiSpec, TransactTo, TxEnv};

pub fn env_for_block(block_id: u64, block_timestamp: u64) -> Env {
    let mut env = Env::default();
    env.block.timestamp = U256::from(block_timestamp);
    env.block.number = U256::from(block_id);
    env
}

pub fn evm_call(state_db: &InMemoryDB, env: Env, transact_to: Address, call_data_vec: Vec<u8>) -> Result<(Vec<u8>, u64)> {
    let mut env = env;

    env.tx.transact_to = TransactTo::Call(transact_to);
    env.tx.data = Bytes::from(call_data_vec);
    env.tx.value = U256::from(0);


    let mut evm = Evm::new(
        Context { evm: EvmContext::new_with_env(WrapDatabaseRef(state_db), Box::new(env)), external: () }, Handler::mainnet::<ShanghaiSpec>());

    let ref_tx = evm.transact()?;
    let result = ref_tx.result;


    let gas_used = result.gas_used();

    // unpack output call enum into raw bytes
    let value = match result {
        ExecutionResult::Success { output, .. } => match output {
            Output::Call(value) => Some(value),
            _ => None,
        },
        ExecutionResult::Revert { output, gas_used } => {
            trace!("Revert {} : {:?}", gas_used, output);
            None
        }

        _ => None,
    };
    match value {
        Some(v) => {
            Ok((v.to_vec(), gas_used))
        }
        None => Err(eyre!("CALL_RESULT_IS_EMPTY"))
    }
}

pub fn evm_transact(evm: &mut Evm<(), InMemoryDB>, tx: &Transaction) -> Result<()>
{
    let env = evm.env_mut();

    env.tx.transact_to = TransactTo::Call(tx.to.unwrap());
    env.tx.nonce = Some(tx.nonce);
    env.tx.data = tx.input.clone();
    env.tx.value = tx.value;
    env.tx.caller = tx.from;
    env.tx.gas_price = U256::from(tx.gas_price.unwrap_or_default());
    env.tx.gas_limit = tx.gas as u64;
    env.tx.gas_priority_fee = Some(U256::from(tx.max_priority_fee_per_gas.unwrap_or_default()));


    match evm.transact_commit() {
        Ok(execution_result) => {
            match execution_result {
                ExecutionResult::Success { output, gas_used, reason, .. } => {
                    debug!("Transact Gas used : {gas_used} reason:  {reason:?}");
                    debug!("Transact Output : {output:?}");

                    Ok(())
                }
                ExecutionResult::Revert { output, gas_used } => {
                    error!("Revert {output}");
                    Err(eyre!("EXECUTION_REVERTED"))
                }
                ExecutionResult::Halt { reason, .. } => {
                    error!("Halt {reason:?}");
                    Err(eyre!("EXECUTION_HALT"))
                }
            }
        }
        Err(e) => {
            error!("Execution error : {e}");
            Err(eyre!("EXECUTION_ERROR"))
        }
    }
}


lazy_static! {
    static ref COINBASE : Address = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326".parse().unwrap();
}

pub fn evm_access_list(state_db: &InMemoryDB, env: &Env, tx: &TransactionRequest) -> Result<(u64, AccessList)>
{
    let txto = tx.to.unwrap_or_default().to().map_or(Address::ZERO, |x| *x);

    let mut env = env.clone();
    env.tx.chain_id = tx.chain_id;
    env.tx.transact_to = TransactTo::Call(txto);
    env.tx.nonce = tx.nonce;
    env.tx.data = tx.input.clone().data.unwrap();
    env.tx.value = tx.value.unwrap_or_default();
    env.tx.caller = tx.from.unwrap_or_default();
    env.tx.gas_limit = tx.gas.unwrap_or_default() as u64;
    env.tx.gas_price = U256::from(tx.max_fee_per_gas.unwrap_or_default());
    env.tx.gas_priority_fee = Some(U256::from(tx.max_priority_fee_per_gas.unwrap_or_default()));

    env.block.coinbase = *COINBASE;

    let mut evm = Evm::builder().with_ref_db(state_db).with_spec_id(SHANGHAI).with_env(Box::new(env)).build();

    match evm.transact() {
        Ok(execution_result) => {
            match execution_result.result {
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
            }
        }
        Err(e) => {
            error!("Execution error : {e}");
            Err(eyre!("EXECUTION_ERROR"))
        }
    }
}

fn evm_env_from_tx<T: Into<Transaction>>(tx: T, block_header: Header) -> Env {
    let tx = tx.into();

    let blob_gas = if block_header.blob_gas_used.is_some() && block_header.excess_blob_gas.is_some() {
        Some(BlobExcessGasAndPrice {
            excess_blob_gas: block_header.blob_gas_used.unwrap_or_default() as u64,
            blob_gasprice: block_header.excess_blob_gas.unwrap_or_default(),
        })
    } else {
        None
    };

    Env {
        cfg: Default::default(),
        block: BlockEnv {
            number: U256::from(block_header.number.unwrap_or_default()),
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
        },
    }
}

#[cfg(test)]
mod test
{
    use revm::db::{CacheDB, EmptyDB};

    use super::*;

    #[test]
    fn test_transact() {
        let db = CacheDB::new(EmptyDB::new());
        let env = Env::default();
        //evm_transact(db, env, Address::repeat_byte(1), vec![]);
    }
}