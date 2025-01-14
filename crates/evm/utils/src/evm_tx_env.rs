use alloy::consensus::Transaction as TransactionTrait;
use alloy::consensus::{TxEip4844Variant, TxEnvelope};
use alloy::primitives::private::alloy_rlp;
use alloy::primitives::{Bytes, SignatureError, TxKind, U256};
use alloy::rlp::Decodable;
use alloy::rpc::types::Transaction;
use revm::primitives::{AuthorizationList, TxEnv};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnvError {
    #[error(transparent)]
    AlloyRplError(#[from] alloy_rlp::Error),
    #[error(transparent)]
    SignatureError(#[from] SignatureError),
    #[error("Unsupported transaction type")]
    UnsupportedTransactionType,
}

pub fn env_from_signed_tx(rpl_bytes: Bytes) -> Result<TxEnv, EnvError> {
    match TxEnvelope::decode(&mut rpl_bytes.iter().as_slice())? {
        TxEnvelope::Legacy(tx) => {
            Ok(TxEnv {
                caller: tx.recover_signer()?,
                transact_to: tx.tx().to,
                nonce: Some(tx.tx().nonce),
                data: tx.tx().input.clone(),
                value: tx.tx().value,
                gas_price: U256::from(tx.tx().gas_price),
                gas_limit: tx.tx().gas_limit,
                chain_id: tx.tx().chain_id,

                // not supported
                access_list: vec![],
                gas_priority_fee: None,
                blob_hashes: vec![],
                max_fee_per_blob_gas: None,
                authorization_list: None,
            })
        }
        TxEnvelope::Eip2930(tx) => {
            Ok(TxEnv {
                caller: tx.recover_signer()?,
                transact_to: tx.tx().to,
                nonce: Some(tx.tx().nonce),
                data: tx.tx().input.clone(),
                value: tx.tx().value,
                gas_price: U256::from(tx.tx().gas_price),
                gas_limit: tx.tx().gas_limit,
                chain_id: Some(tx.tx().chain_id),
                access_list: tx.tx().clone().access_list.0,

                // not supported
                gas_priority_fee: None,
                blob_hashes: vec![],
                max_fee_per_blob_gas: None,
                authorization_list: None,
            })
        }
        TxEnvelope::Eip1559(tx) => {
            Ok(TxEnv {
                caller: tx.recover_signer()?,
                transact_to: tx.tx().to,
                nonce: Some(tx.tx().nonce),
                data: tx.tx().input.clone(),
                value: tx.tx().value,
                gas_price: U256::from(tx.tx().max_fee_per_gas),
                gas_priority_fee: Some(U256::from(tx.tx().max_priority_fee_per_gas)),
                gas_limit: tx.tx().gas_limit,
                chain_id: Some(tx.tx().chain_id),
                access_list: tx.tx().clone().access_list.0,

                // not supported
                blob_hashes: vec![],
                max_fee_per_blob_gas: None,
                authorization_list: None,
            })
        }
        TxEnvelope::Eip4844(signed_tx) => {
            let tx = match signed_tx.tx() {
                TxEip4844Variant::TxEip4844(tx) => tx,
                TxEip4844Variant::TxEip4844WithSidecar(tx) => tx.tx(),
            };
            Ok(TxEnv {
                caller: signed_tx.recover_signer()?,
                transact_to: TxKind::Call(tx.to),
                nonce: Some(tx.nonce),
                data: tx.input.clone(),
                value: tx.value,
                gas_price: U256::from(tx.max_fee_per_gas),
                gas_priority_fee: Some(U256::from(tx.max_priority_fee_per_gas)),
                gas_limit: tx.gas_limit,
                chain_id: Some(tx.chain_id),
                access_list: tx.clone().access_list.0,
                max_fee_per_blob_gas: Some(U256::from(tx.max_fee_per_blob_gas)),
                blob_hashes: tx.blob_versioned_hashes.clone(),

                // Not supported
                authorization_list: None,
            })
        }
        TxEnvelope::Eip7702(tx) => {
            Ok(TxEnv {
                caller: tx.recover_signer()?,
                transact_to: TxKind::Call(tx.tx().to),
                nonce: Some(tx.tx().nonce),
                data: tx.tx().input.clone(),
                value: tx.tx().value,
                gas_price: U256::from(tx.tx().max_fee_per_gas),
                gas_priority_fee: Some(U256::from(tx.tx().max_priority_fee_per_gas)),
                gas_limit: tx.tx().gas_limit,
                chain_id: Some(tx.tx().chain_id),
                access_list: tx.tx().clone().access_list.0,
                authorization_list: Some(AuthorizationList::Signed(tx.tx().clone().authorization_list)),

                // Not supported
                blob_hashes: vec![],
                max_fee_per_blob_gas: None,
            })
        }
    }
}

pub fn tx_to_evm_tx(tx: &Transaction) -> TxEnv {
    TxEnv {
        transact_to: match tx.to() {
            Some(to) => TxKind::Call(to),
            None => TxKind::Create,
        },
        nonce: Some(tx.nonce()),
        chain_id: tx.chain_id(),
        data: tx.input().clone(),
        value: tx.value(),
        caller: tx.from,
        gas_limit: tx.gas_limit(),

        // support type 1 and 2
        gas_price: U256::from(tx.max_fee_per_gas()),
        gas_priority_fee: Some(U256::from(tx.max_priority_fee_per_gas().unwrap_or_default())),

        // Not used in loom context
        blob_hashes: vec![],
        max_fee_per_blob_gas: None,
        access_list: vec![],
        authorization_list: None,
    }
}
