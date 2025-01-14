use crate::handler::blocks::latest_block;
use crate::handler::flashbots::flashbots;
use crate::handler::pools::{market_stats, pool, pool_quote, pools};
use crate::handler::ws::ws_handler;
//use crate::openapi::ApiDoc;
use axum::routing::{get, post};
use axum::Router;
use eyre::ErrReport;
use loom_rpc_state::AppState;
use revm::{DatabaseCommit, DatabaseRef};
//use utoipa::OpenApi;
//use utoipa_swagger_ui::SwaggerUi;

pub fn router<DB: DatabaseRef<Error = ErrReport> + DatabaseCommit + Sync + Send + Clone + 'static>(app_state: AppState<DB>) -> Router<()> {
    Router::new()
        .nest(
            "/api/v1",
            Router::new()
                .nest("/block", router_block()) // rename to node
                .nest("/markets", router_market())
                .nest("/flashbots", Router::new().route("/", post(flashbots))),
        )
        .route("/ws", get(ws_handler))
        //.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(app_state)
}

pub fn router_block<DB: DatabaseRef + DatabaseCommit + Sync + Send + Clone + 'static>() -> Router<AppState<DB>> {
    Router::new().route("/latest_block", get(latest_block))
}

pub fn router_market<DB: DatabaseRef<Error = ErrReport> + DatabaseCommit + Sync + Send + Clone + 'static>() -> Router<AppState<DB>> {
    Router::new()
        .route("/pools/:address", get(pool))
        .route("/pools/:address/quote", post(pool_quote))
        .route("/pools", get(pools))
        .route("/", get(market_stats))
}
