use alloy::consensus::Transaction as TransactionTrait;
use alloy::primitives::U256;
use alloy::rpc::types::{Header, Transaction};
use revm::primitives::{BlockEnv, Env, TransactTo, TxEnv};

pub fn env_for_block(block_id: u64, block_timestamp: u64) -> Env {
    let mut env = Env::default();
    env.block.timestamp = U256::from(block_timestamp);
    env.block.number = U256::from(block_id);
    env
}

pub fn evm_env_from_tx<T: Into<Transaction>>(tx: T, block_header: &Header) -> Env {
    let tx = tx.into();

    Env {
        cfg: Default::default(),
        block: BlockEnv {
            number: U256::from(block_header.number),
            coinbase: block_header.beneficiary,
            timestamp: U256::from(block_header.timestamp),
            gas_limit: U256::from(block_header.gas_limit),
            basefee: U256::from(block_header.base_fee_per_gas.unwrap_or_default()),
            difficulty: block_header.difficulty,
            prevrandao: Some(block_header.parent_hash),
            blob_excess_gas_and_price: None,
        },
        tx: TxEnv {
            caller: tx.from,
            gas_limit: tx.gas_limit(),
            gas_price: U256::from(tx.max_fee_per_gas()),
            transact_to: TransactTo::Call(tx.to().unwrap_or_default()),
            value: tx.value(),
            data: tx.input().clone(),
            nonce: Some(tx.nonce()),
            chain_id: tx.chain_id(),
            access_list: Vec::new(),
            gas_priority_fee: tx.max_priority_fee_per_gas().map(|x| U256::from(x)),
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: None,
            authorization_list: None,
        },
    }
}
