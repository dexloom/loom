use crate::dto::block::BlockHeader;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use loom_rpc_state::AppState;
use revm::{DatabaseCommit, DatabaseRef};

/// Get latest block
///
/// Get the latest block header
#[utoipa::path(
    get,
    path = "latest_block",
    tag = "block",
    tags = [],
    responses(
    (status = 200, description = "Todo item created successfully", body = BlockHeader),
    )
)]
pub async fn latest_block<DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static>(
    State(app_state): State<AppState<DB>>,
) -> Result<Json<BlockHeader>, (StatusCode, String)> {
    {
        let block_header_opt = app_state.bc.latest_block().read().await.block_header.clone();
        if let Some(block_header) = block_header_opt {
            let ret = BlockHeader {
                number: block_header.number,
                timestamp: block_header.timestamp,
                base_fee_per_gas: block_header.base_fee_per_gas,
                next_block_base_fee: 0,
            };
            Ok(Json(ret))
        } else {
            Err((StatusCode::INTERNAL_SERVER_ERROR, "No block header found".to_string()))
        }
    }
}
