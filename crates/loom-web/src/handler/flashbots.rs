use crate::dto::flashbots::{BundleRequest, BundleResponse, SendBundleResponse};
use alloy_consensus::TxEnvelope;
use alloy_primitives::private::alloy_rlp;
use alloy_primitives::private::alloy_rlp::Decodable;
use alloy_primitives::{Bytes, SignatureError, TxKind, U256};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use loom_utils::evm::{env_for_block, evm_call};
use loom_web_state::AppState;
use revm::primitives::TxEnv;
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum EnvError {
    #[error(transparent)]
    AlloyRplError(#[from] alloy_rlp::Error),
    #[error(transparent)]
    SignatureError(#[from] SignatureError),
}

pub async fn flashbots(
    State(app_state): State<AppState>,
    Json(bundle_request): Json<BundleRequest>,
) -> Result<Json<SendBundleResponse>, (StatusCode, String)> {
    for (bundle_idx, bundle_param) in bundle_request.params.iter().enumerate() {
        info!(
            "Flashbots bundle({bundle_idx}): target_block={:?}, transactions_len={:?}",
            bundle_param.target_block,
            bundle_param.transactions.len()
        );
        for (tx_idx, tx) in bundle_param.transactions.iter().enumerate() {
            let mut tx_env = TxEnv::default();
            env_from_signed_tx(&mut tx_env, tx.clone()).map_err(|e| (StatusCode::BAD_REQUEST, format!("Error: {}", e)))?;
            info!("Flashbots bundle({bundle_idx}) -> tx({tx_idx}): caller={:?}, transact_to={:?}, data={:?}, value={:?}, gas_price={:?}, gas_limit={:?}, nonce={:?}, chain_id={:?}, access_list_len={}",
               tx_env.caller, tx_env.transact_to, tx_env.data, tx_env.value, tx_env.gas_price, tx_env.gas_limit, tx_env.nonce, tx_env.chain_id, tx_env.access_list.len());

            let last_block_header = app_state.bc.latest_block().read().await.block_header.clone().unwrap_or_default();
            let target_block = bundle_param.target_block.unwrap_or_default().to::<u64>();
            if target_block != last_block_header.number + 1 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Target block is not next block: {} != {}", target_block, last_block_header.number + 1),
                ));
            }
            let mut evm_env = env_for_block(last_block_header.number + 1, last_block_header.timestamp + 12);
            evm_env.tx = tx_env;
            let transact_to = match evm_env.tx.transact_to {
                TxKind::Create => {
                    return Err((StatusCode::BAD_REQUEST, "Create contract is not supported".to_string()));
                }
                TxKind::Call(caller) => caller,
            };
            let call_data_vec = evm_env.tx.data.0.to_vec();
            let (result, gas_used) = evm_call(&app_state.bc.market_state().read().await.state_db, evm_env, transact_to, call_data_vec)
                .map_err(|e| {
                    error!("Flashbot tx error: {}", e);
                    (StatusCode::BAD_REQUEST, format!("Error: {}", e))
                })?;
            info!("result: {:?}, gas_used: {}", result, gas_used);
        }
    }

    Ok(Json(SendBundleResponse { jsonrpc: "2.0".to_string(), id: 1, result: BundleResponse { bundle_hash: None } }))
}

pub fn env_from_signed_tx(tx_env: &mut TxEnv, rpl_bytes: Bytes) -> Result<(), EnvError> {
    match TxEnvelope::decode(&mut rpl_bytes.iter().as_slice())? {
        TxEnvelope::Legacy(_) => {
            todo!("Legacy transactions are not supported")
        }
        TxEnvelope::Eip2930(_) => {
            todo!("EIP-2930 transactions are not supported")
        }
        TxEnvelope::Eip1559(tx) => {
            match tx.recover_signer() {
                Ok(signer) => {
                    tx_env.caller = signer;
                }
                Err(e) => {
                    return Err(EnvError::SignatureError(e));
                }
            }

            tx_env.transact_to = tx.tx().to;
            tx_env.data = tx.tx().input.clone();
            tx_env.value = tx.tx().value;
            tx_env.gas_price = U256::from(tx.tx().max_fee_per_gas);
            tx_env.gas_priority_fee = Some(U256::from(tx.tx().max_priority_fee_per_gas));
            tx_env.gas_limit = tx.tx().gas_limit;
            tx_env.nonce = Some(tx.tx().nonce);
            tx_env.chain_id = Some(tx.tx().chain_id);
            tx_env.access_list = tx.tx().clone().access_list.0;
            Ok(())
        }
        TxEnvelope::Eip4844(_) => {
            todo!("EIP-4844 transactions are not supported")
        }
        _ => {
            todo!("Unknown transaction type")
        }
    }
}
