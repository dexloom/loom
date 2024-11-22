use crate::router::router;
use axum::Router;
use eyre::ErrReport;
use loom_core_actors::{Actor, ActorResult, WorkerResult};
use loom_core_actors_macros::Consumer;
use loom_core_blockchain::{Blockchain, BlockchainState};
use loom_rpc_state::AppState;
use loom_storage_db::DbPool;
use revm::{DatabaseCommit, DatabaseRef};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::info;

pub async fn start_web_server_worker<S, DB>(
    host: String,
    extra_router: Router<S>,
    bc: Blockchain,
    state: BlockchainState<DB>,
    db_pool: DbPool,
    shutdown_token: CancellationToken,
) -> WorkerResult
where
    DB: DatabaseRef<Error = ErrReport> + DatabaseCommit + Send + Sync + Clone + Default + 'static,
    S: Clone + Send + Sync + 'static,
    Router: From<Router<S>>,
{
    let app_state = AppState { db: db_pool, bc, state };
    let router = router(app_state);
    let router = router.merge(extra_router);

    // logging
    let router = router.layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default().include_headers(true)));

    info!("Webserver listening on {}", &host);
    let listener = TcpListener::bind(host).await?;
    axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async move {
            shutdown_token.cancelled().await;
            info!("Shutting down webserver...");
        })
        .await?;

    Ok("Webserver shutdown".to_string())
}

#[derive(Consumer)]
pub struct WebServerActor<S, DB: Clone + Send + Sync + 'static> {
    host: String,
    extra_router: Router<S>,
    shutdown_token: CancellationToken,
    db_pool: DbPool,
    bc: Option<Blockchain>,
    state: Option<BlockchainState<DB>>,
}

impl<S, DB> WebServerActor<S, DB>
where
    DB: DatabaseRef<Error = ErrReport> + Send + Sync + Clone + Default + 'static,
    S: Clone + Send + Sync + 'static,
    Router: From<Router<S>>,
{
    pub fn new(host: String, extra_router: Router<S>, db_pool: DbPool, shutdown_token: CancellationToken) -> Self {
        Self { host, extra_router, shutdown_token, db_pool, bc: None, state: None }
    }

    pub fn on_bc(self, bc: &Blockchain, state: &BlockchainState<DB>) -> Self {
        Self { bc: Some(bc.clone()), state: Some(state.clone()), ..self }
    }
}

impl<S, DB> Actor for WebServerActor<S, DB>
where
    S: Clone + Send + Sync + 'static,
    Router: From<Router<S>>,
    DB: DatabaseRef<Error = ErrReport> + DatabaseCommit + Send + Sync + Clone + Default + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::spawn(start_web_server_worker(
            self.host.clone(),
            self.extra_router.clone(),
            self.bc.clone().unwrap(),
            self.state.clone().unwrap(),
            self.db_pool.clone(),
            self.shutdown_token.clone(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "WebServerActor"
    }
}
