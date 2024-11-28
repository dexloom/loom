use crate::dto::flashbots::{BundleRequest, BundleResponse, SendBundleResponse};
use alloy_primitives::{hex, keccak256, U256};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use loom_evm_utils::evm::evm_transact;
use loom_evm_utils::evm_tx_env::env_from_signed_tx;
use loom_rpc_state::AppState;
use loom_types_blockchain::ChainParameters;
use revm::primitives::{BlockEnv, Env, CANCUN};
use revm::{DatabaseCommit, DatabaseRef, Evm};
use std::fmt::Debug;
use tracing::{error, info};

pub async fn flashbots<DB>(
    State(app_state): State<AppState<DB>>,
    Json(bundle_request): Json<BundleRequest>,
) -> Result<Json<SendBundleResponse>, (StatusCode, String)>
where
    DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static,
    <DB as DatabaseRef>::Error: Debug,
{
    for (bundle_idx, bundle_param) in bundle_request.params.iter().enumerate() {
        info!(
            "Flashbots bundle({bundle_idx}): target_block={:?}, transactions_len={:?}",
            bundle_param.target_block,
            bundle_param.transactions.len()
        );
        let last_block_header = app_state.bc.latest_block().read().await.block_header.clone().unwrap_or_default();
        let target_block = bundle_param.target_block.unwrap_or_default().to::<u64>();
        if target_block <= last_block_header.number {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Target block is target_block={} <= last_block={}", target_block, last_block_header.number),
            ));
        }
        let next_block_timestamp = last_block_header.timestamp + 12 * (target_block - last_block_header.number);
        let next_block_base_fee = ChainParameters::ethereum().calc_next_block_base_fee(
            last_block_header.gas_used,
            last_block_header.gas_limit,
            last_block_header.base_fee_per_gas.unwrap_or_default(),
        );
        let evm_env = Env {
            block: BlockEnv {
                number: U256::from(target_block),
                timestamp: U256::from(next_block_timestamp),
                basefee: U256::from(next_block_base_fee),
                ..BlockEnv::default()
            },
            ..Env::default()
        };
        let db = app_state.state.market_state().read().await.state_db.clone();
        let mut evm = Evm::builder().with_spec_id(CANCUN).with_ref_db(db).with_env(Box::new(evm_env)).build();
        for (tx_idx, tx) in bundle_param.transactions.iter().enumerate() {
            let tx_hash = keccak256(tx);

            let tx_env = env_from_signed_tx(tx.clone()).map_err(|e| (StatusCode::BAD_REQUEST, format!("Error: {}", e)))?;
            info!("Flashbots bundle({bundle_idx}) -> tx({tx_idx}): caller={:?}, transact_to={:?}, data={:?}, value={:?}, gas_price={:?}, gas_limit={:?}, nonce={:?}, chain_id={:?}, access_list_len={}",
               tx_env.caller, tx_env.transact_to, tx_env.data, tx_env.value, tx_env.gas_price, tx_env.gas_limit, tx_env.nonce, tx_env.chain_id, tx_env.access_list.len());

            evm.context.evm.env.tx = tx_env;

            let (result, gas_used) = evm_transact(&mut evm).map_err(|e| {
                error!("Flashbot tx error latest_block={}, tx_hash={}, err={}/{:?}", last_block_header.number, tx_hash, e, e);
                (StatusCode::BAD_REQUEST, format!("Error: {}", e))
            })?;
            info!("result={}, gas_used={}", hex::encode_prefixed(result), gas_used);
        }
    }

    Ok(Json(SendBundleResponse { jsonrpc: "2.0".to_string(), id: 1, result: BundleResponse { bundle_hash: None } }))
}
