use crate::dto::block::BlockHeader;
use crate::dto::pool::MarketStats;
use crate::dto::pool::Pool;
use crate::dto::pool::PoolClass;
use crate::dto::pool::PoolDetailsResponse;
use crate::dto::pool::PoolProtocol;
use crate::dto::pool::PoolResponse;
use crate::dto::quote::QuoteRequest;
use crate::dto::quote::QuoteResponse;
use crate::handler::blocks::__path_latest_block;
use crate::handler::pools::__path_market_stats;
use crate::handler::pools::__path_pool;
use crate::handler::pools::__path_pool_quote;
use crate::handler::pools::__path_pools;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(latest_block),
    tags(
        (name = "block", description = "Blockchain")
    ),
    components(schemas(BlockHeader))
)]
pub struct BlockApi;

#[derive(OpenApi)]
#[openapi(
    paths(pool, pools, pool_quote, market_stats),
    tags(
        (name = "market", description = "Market")
    ),
    components(schemas(PoolResponse, PoolDetailsResponse, Pool, PoolClass, PoolProtocol, MarketStats, QuoteRequest, QuoteResponse))
)]
pub struct MarketApi;

#[allow(dead_code)]
#[derive(OpenApi)]
#[openapi(
    nest(
        (path = "/api/v1/block/", api = BlockApi),
        (path = "/api/v1/markets", api = MarketApi)
    )
)]
pub struct ApiDoc;
