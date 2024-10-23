use crate::router::router;
use axum::Router;
use defi_blockchain::Blockchain;
use loom_actors::{Actor, ActorResult, WorkerResult};
use loom_actors_macros::Consumer;
use loom_db::init_db_pool;
use loom_web_state::AppState;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::info;

pub async fn start_web_server_worker<S>(
    host: String,
    extra_router: Router<S>,
    bc: Blockchain,
    shutdown_token: CancellationToken,
) -> WorkerResult
where
    S: Clone + Send + Sync + 'static,
    Router: From<Router<S>>,
{
    let db_pool = init_db_pool().await?;
    let app_state = AppState { db: db_pool, bc };
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
pub struct WebServerActor<S> {
    host: String,
    router: Router<S>,
    shutdown_token: CancellationToken,
    bc: Option<Blockchain>,
}

impl<S> WebServerActor<S>
where
    S: Clone + Send + Sync + 'static,
    Router: From<Router<S>>,
{
    pub fn new(host: String, router: Router<S>, shutdown_token: CancellationToken) -> Self {
        Self { host, router, shutdown_token, bc: None }
    }

    pub fn on_bc(self, bc: Blockchain) -> Self {
        Self { bc: Some(bc.clone()), ..self }
    }
}

impl<S> Actor for WebServerActor<S>
where
    S: Clone + Send + Sync + 'static,
    Router: From<Router<S>>,
{
    fn start(&self) -> ActorResult {
        let task = tokio::spawn(start_web_server_worker(
            self.host.clone(),
            self.router.clone(),
            self.bc.clone().unwrap(),
            self.shutdown_token.clone(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "WebServerActor"
    }
}
