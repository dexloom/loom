use crate::dto::pagination::Pagination;
use crate::dto::pool::{MarketStats, Pool, PoolClass, PoolDetailsResponse, PoolProtocol, PoolResponse};
use crate::dto::quote::{Filter, QuoteRequest, QuoteResponse};
use alloy_primitives::Address;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use eyre::ErrReport;
use loom_evm_utils::error_handler::internal_error;
use loom_rpc_state::AppState;
use loom_types_entities::{PoolId, PoolWrapper};
use revm::primitives::Env;
use revm::{DatabaseCommit, DatabaseRef};
use std::str::FromStr;

/// Get latest block
///
/// Get the latest block header
#[utoipa::path(
    get,
    path = "/pools",
    tag = "market",
    tags = [],
    params(
        Pagination, Filter
    ),
    responses(
    (status = 200, description = "All available pools", body = PoolResponse),
    )
)]
pub async fn pools<DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static>(
    State(app_state): State<AppState<DB>>,
    pagination: Query<Pagination>,
    filter: Query<Filter>,
) -> Result<Json<PoolResponse>, (StatusCode, String)> {
    let pools: Vec<(Address, PoolWrapper)> = app_state
        .bc
        .market()
        .read()
        .await
        .pools()
        .iter()
        .filter(|(_, pool)| match &filter.protocol {
            None => true,
            Some(protocol) => pool.pool.get_protocol() == protocol.into(),
        })
        .skip(pagination.start())
        .take(pagination.limit)
        .map(|(address, pool)| (address.address_or_zero(), pool.clone()))
        .collect();

    let mut ret = vec![];
    for (pool_address, pool) in pools {
        ret.push(Pool {
            address: pool_address,
            fee: pool.pool.get_fee(),
            tokens: pool.pool.get_tokens(),
            protocol: PoolProtocol::from(pool.pool.get_protocol()),
            pool_class: PoolClass::from(pool.get_class()),
        });
    }
    let total_pools = app_state
        .bc
        .market()
        .read()
        .await
        .pools()
        .iter()
        .filter(|(_, pool)| match &filter.protocol {
            None => true,
            Some(protocol) => pool.pool.get_protocol() == protocol.into(),
        })
        .count();

    Ok(Json(PoolResponse { pools: ret, total: total_pools }))
}

/// Get pool details
///
/// Get pool details
#[utoipa::path(
    get,
    path = "/pools/{address}",
    tag = "market",
    tags = [],
    params(
        ("address" = String, Path, description = "Address of the pool"),
    ),
    responses(
    (status = 200, description = "Pool detail response", body = PoolDetailsResponse),
    )
)]
pub async fn pool<DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static>(
    State(app_state): State<AppState<DB>>,
    Path(address): Path<String>,
) -> Result<Json<PoolDetailsResponse>, (StatusCode, String)> {
    let address = Address::from_str(&address).map_err(internal_error)?;

    match app_state.bc.market().read().await.pools().get(&PoolId::Address(address)) {
        None => Err((StatusCode::NOT_FOUND, "Pool not found".to_string())),
        Some(pool) => Ok(Json(PoolDetailsResponse {
            address: pool.get_address(),
            pool_class: PoolClass::from(pool.get_class()),
            protocol: PoolProtocol::from(pool.get_protocol()),
            fee: pool.get_fee(),
            tokens: pool.get_tokens(),
        })),
    }
}

/// Market statistics
///
/// Get the latest market statistics
#[utoipa::path(
    get,
    path = "/stats",
    tag = "market",
    tags = [],
    params(
        Pagination, Filter
    ),
    responses(
        (status = 200, description = "Market stats", body = MarketStats),
    )
)]
pub async fn market_stats<DB: DatabaseRef + DatabaseCommit + Send + Sync + Clone + 'static>(
    State(app_state): State<AppState<DB>>,
) -> Result<Json<MarketStats>, (StatusCode, String)> {
    let total_pools = app_state.bc.market().read().await.pools().len();

    Ok(Json(MarketStats { total_pools }))
}

/// Get a quote
///
/// Get quote for a pair of a pool
#[utoipa::path(
    post,
    path = "/pools/{address}/quote",
    tag = "market",
    tags = [],
    params(
        ("address" = String, Path, description = "Address of the pool"),
    ),
    request_body = QuoteRequest,
    responses(
        (status = 200, description = "Market stats", body = QuoteResponse),
    )
)]
pub async fn pool_quote<DB: DatabaseRef<Error = ErrReport> + DatabaseCommit + Send + Sync + Clone + 'static>(
    State(app_state): State<AppState<DB>>,
    Path(address): Path<String>,
    Json(quote_request): Json<QuoteRequest>,
) -> Result<Json<QuoteResponse>, (StatusCode, String)> {
    let address = Address::from_str(&address).map_err(internal_error)?;
    match app_state.bc.market().read().await.pools().get(&PoolId::Address(address)) {
        None => Err((StatusCode::NOT_FOUND, "Pool not found".to_string())),
        Some(pool) => {
            let evm_env = Env::default();
            let quote_result = pool.pool.calculate_out_amount(
                &app_state.state.market_state().read().await.state_db,
                evm_env,
                &quote_request.token_address_from,
                &quote_request.token_address_to,
                quote_request.amount_in,
            );
            match quote_result {
                Err(err) => Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
                Ok((out_amount, gas_used)) => Ok(Json(QuoteResponse { out_amount, gas_used })),
            }
        }
    }
}
