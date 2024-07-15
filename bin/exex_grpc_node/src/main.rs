use example_exex_remote::proto::{
    ExExNotification as ProtoExExNotification,
    remote_ex_ex_server::{RemoteExEx, RemoteExExServer}, SubscribeRequest as ProtoSubscribeRequest,
    Transaction as ProtoTransaction,
};
use reth::primitives::{IntoRecoveredTransaction, Transaction, TransactionSigned};
use reth::transaction_pool::{
    BlobStore, Pool, TransactionOrdering, TransactionPool, TransactionValidator,
};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_tracing::tracing::{error, info};
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};

#[derive(Debug)]
struct ExExService {
    notifications_exex: broadcast::Sender<ExExNotification>,
    notifications_tx: broadcast::Sender<TransactionSigned>,
}

#[tonic::async_trait]
impl RemoteExEx for ExExService {
    type SubscribeExExStream = ReceiverStream<Result<ProtoExExNotification, Status>>;
    type SubscribeMempoolTxStream = ReceiverStream<Result<ProtoTransaction, Status>>;

    async fn subscribe_ex_ex(
        &self,
        _request: Request<ProtoSubscribeRequest>,
    ) -> Result<Response<Self::SubscribeExExStream>, Status> {
        let (tx, rx) = mpsc::channel(1);

        let mut notifications = self.notifications_exex.subscribe();
        tokio::spawn(async move {
            while let Ok(notification) = notifications.recv().await {
                if let Err(e) = tx
                    .send(Ok((&notification).try_into().expect("failed to encode")))
                    .await
                {
                    error!(error=?e , "exex.send");
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe_mempool_tx(
        &self,
        _request: Request<ProtoSubscribeRequest>,
    ) -> Result<Response<Self::SubscribeMempoolTxStream>, Status> {
        let (tx, rx) = mpsc::channel(1);

        let mut notifications = self.notifications_tx.subscribe();
        tokio::spawn(async move {
            while let Ok(notification) = notifications.recv().await {
                if let Err(e) = tx
                    .send(Ok((&notification).try_into().expect("failed to encode")))
                    .await
                {
                    error!(error=?e,"mempool.send");
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

async fn exex<Node: FullNodeComponents>(
    mut ctx: ExExContext<Node>,
    notifications: broadcast::Sender<ExExNotification>,
) -> eyre::Result<()> {
    while let Some(notification) = ctx.notifications.recv().await {
        if let Some(committed_chain) = notification.committed_chain() {
            ctx.events
                .send(ExExEvent::FinishedHeight(committed_chain.tip().number))?;
        }

        let _ = notifications.send(notification);
    }

    Ok(())
}

pub async fn mempool_worker<V, T, S>(
    mempool: Pool<V, T, S>,
    notifications: broadcast::Sender<TransactionSigned>,
) -> eyre::Result<()>
where
    V: TransactionValidator,
    T: TransactionOrdering<Transaction=<V as TransactionValidator>::Transaction>,
    S: BlobStore,
{
    info!("Mempool worker started");
    let mut tx_listener = mempool.new_transactions_listener();

    while let Some(tx_notification) = tx_listener.recv().await {
        let _ = notifications.send(
            tx_notification
                .transaction
                .to_recovered_transaction()
                .into(),
        );
    }

    Ok(())
}

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let notifications_exex = broadcast::channel(1).0;
        let notifications_tx = broadcast::channel(1).0;

        let server = Server::builder()
            .add_service(RemoteExExServer::new(ExExService {
                notifications_exex: notifications_exex.clone(),
                notifications_tx: notifications_tx.clone(),
            }))
            .serve("[::1]:10000".parse().unwrap());

        let handle = builder
            .node(EthereumNode::default())
            .install_exex(
                "Remote",
                |ctx| async move { Ok(exex(ctx, notifications_exex)) },
            )
            .launch()
            .await?;

        let mempool = handle.node.pool.clone();

        tokio::task::spawn(mempool_worker(mempool, notifications_tx));

        handle
            .node
            .task_executor
            .spawn_critical("gRPC server", async move {
                server.await.expect("gRPC server crashed")
            });

        handle.wait_for_node_exit().await
    })
}
