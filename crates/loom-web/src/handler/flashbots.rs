use crate::dto::flashbots::{BundleRequest, BundleResponse, SendBundleResponse};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use loom_utils::evm::evm_transact;
use loom_utils::evm_env::env_for_block;
use loom_utils::evm_tx_env::env_from_signed_tx;
use loom_web_state::AppState;
use revm::primitives::SHANGHAI;
use revm::Evm;
use tracing::{error, info};

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
        let last_block_header = app_state.bc.latest_block().read().await.block_header.clone().unwrap_or_default();
        let target_block = bundle_param.target_block.unwrap_or_default().to::<u64>();
        if target_block <= last_block_header.number {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Target block is target_block={} <= last_block={}", target_block, last_block_header.number),
            ));
        }
        let block_timestamp = if target_block == last_block_header.number + 1 {
            last_block_header.timestamp + 12
        } else {
            last_block_header.timestamp + 12 * (target_block - last_block_header.number)
        };
        let evm_env = env_for_block(target_block, block_timestamp);
        let db = app_state.bc.market_state().read().await.state_db.clone();
        let mut evm = Evm::builder().with_spec_id(SHANGHAI).with_db(db).with_env(Box::new(evm_env)).build();
        for (tx_idx, tx) in bundle_param.transactions.iter().enumerate() {
            let tx_env = env_from_signed_tx(tx.clone()).map_err(|e| (StatusCode::BAD_REQUEST, format!("Error: {}", e)))?;
            info!("Flashbots bundle({bundle_idx}) -> tx({tx_idx}): caller={:?}, transact_to={:?}, data={:?}, value={:?}, gas_price={:?}, gas_limit={:?}, nonce={:?}, chain_id={:?}, access_list_len={}",
               tx_env.caller, tx_env.transact_to, tx_env.data, tx_env.value, tx_env.gas_price, tx_env.gas_limit, tx_env.nonce, tx_env.chain_id, tx_env.access_list.len());

            evm.context.evm.env.tx = tx_env;

            let (result, gas_used) = evm_transact(&mut evm).map_err(|e| {
                error!("Flashbot tx error: {}", e);
                (StatusCode::BAD_REQUEST, format!("Error: {}", e))
            })?;
            info!("result: {:?}, gas_used: {}", result, gas_used);
        }
    }

    Ok(Json(SendBundleResponse { jsonrpc: "2.0".to_string(), id: 1, result: BundleResponse { bundle_hash: None } }))
}
