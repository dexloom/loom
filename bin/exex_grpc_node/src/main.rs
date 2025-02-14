use reth::api::NodeTypes;
use reth::primitives::{EthPrimitives, TransactionSigned};
use reth::transaction_pool::{EthPooledTransaction, TransactionPool};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_tracing::tracing::{error, info};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};

use loom_node_grpc_exex_proto::proto::{
    ex_ex_notification,
    remote_ex_ex_server::{RemoteExEx, RemoteExExServer},
    Block as ProtoBlock, Chain, ExExNotification as ProtoExExNotification, ReceiptsNotification as ProtoReceiptNotification,
    ReceiptsNotification, SealedHeader as ProtoSealedHeader, StateUpdateNotification as ProtoStateUpdateNotification,
    StateUpdateNotification, SubscribeRequest as ProtoSubscribeRequest, Transaction as ProtoTransaction,
};

#[derive(Debug)]
struct ExExService {
    notifications_exex: broadcast::Sender<ExExNotification>,
    notifications_tx: broadcast::Sender<TransactionSigned>,
}

fn get_chain(notification: ExExNotification) -> Option<Chain> {
    match TryInto::<ProtoExExNotification>::try_into(&notification) {
        Ok(notification) => {
            if let Some(notification) = notification.notification {
                match notification {
                    ex_ex_notification::Notification::ChainCommitted(chain) => chain.new,
                    ex_ex_notification::Notification::ChainReorged(chain) => chain.new,
                    ex_ex_notification::Notification::ChainReverted(_chain) => None,
                }
            } else {
                None
            }
        }
        Err(e) => {
            error!(error=?e , "ExExNotification::try_into");
            None
        }
    }
}

#[tonic::async_trait]
impl RemoteExEx for ExExService {
    type SubscribeExExStream = ReceiverStream<Result<ProtoExExNotification, Status>>;
    type SubscribeMempoolTxStream = ReceiverStream<Result<ProtoTransaction, Status>>;

    type SubscribeHeaderStream = ReceiverStream<Result<ProtoSealedHeader, Status>>;
    type SubscribeBlockStream = ReceiverStream<Result<ProtoBlock, Status>>;
    type SubscribeReceiptsStream = ReceiverStream<Result<ProtoReceiptNotification, Status>>;
    type SubscribeStateUpdateStream = ReceiverStream<Result<ProtoStateUpdateNotification, Status>>;

    async fn subscribe_header(&self, _request: Request<ProtoSubscribeRequest>) -> Result<Response<Self::SubscribeHeaderStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let mut exex_notifications = self.notifications_exex.subscribe();
        tokio::spawn(async move {
            while let Ok(notification) = exex_notifications.recv().await {
                if let Some(chain) = get_chain(notification) {
                    for block in chain.blocks.into_iter() {
                        if let Some(header) = block.header {
                            if let Err(e) = tx.send(Ok(header)).await {
                                error!(error=?e , "header.exex.send");
                                return;
                            }
                        }
                    }
                }
            }
            error!("subscribe_header exex notification loop finished");
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe_block(&self, _request: Request<ProtoSubscribeRequest>) -> Result<Response<Self::SubscribeBlockStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let mut exex_notifications = self.notifications_exex.subscribe();
        tokio::spawn(async move {
            while let Ok(notification) = exex_notifications.recv().await {
                if let Some(chain) = get_chain(notification) {
                    for block in chain.blocks.into_iter() {
                        if let Err(e) = tx.send(Ok(block)).await {
                            error!(error=?e , "blocks.exex.send");
                            return;
                        }
                    }
                }
            }
            error!("subscribe_blocks exex notification loop finished");
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe_receipts(
        &self,
        _request: Request<ProtoSubscribeRequest>,
    ) -> Result<Response<Self::SubscribeReceiptsStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let mut exex_notifications = self.notifications_exex.subscribe();
        tokio::spawn(async move {
            while let Ok(notification) = exex_notifications.recv().await {
                if let Some(chain) = get_chain(notification) {
                    if let Some(execution_outcome) = chain.execution_outcome {
                        for (curblock, receipts) in execution_outcome.receipts.into_iter().enumerate() {
                            let block = chain.blocks[curblock].clone();
                            let receipt_notification = ReceiptsNotification { block: Some(block), receipts: Some(receipts) };
                            if let Err(e) = tx.send(Ok(receipt_notification)).await {
                                error!(error=?e , "receipts.exex.send");
                                return;
                            }
                        }
                    }
                }
            }
            error!("subscribe_blocks exex notification loop finished");
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe_state_update(
        &self,
        _request: Request<ProtoSubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStateUpdateStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let mut exex_notifications = self.notifications_exex.subscribe();
        tokio::spawn(async move {
            while let Ok(notification) = exex_notifications.recv().await {
                if let Some(chain) = get_chain(notification) {
                    if let Some(last_block) = chain.blocks.last() {
                        if let Some(header) = &last_block.header {
                            if let Some(execution_outcome) = chain.execution_outcome {
                                let state_update_notification =
                                    StateUpdateNotification { sealed_header: Some(header.clone()), bundle: execution_outcome.bundle };
                                if let Err(e) = tx.send(Ok(state_update_notification)).await {
                                    error!(error=?e , "state_update.exex.send");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            error!("subscribe_state_update exex notification loop finished");
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe_ex_ex(&self, _request: Request<ProtoSubscribeRequest>) -> Result<Response<Self::SubscribeExExStream>, Status> {
        let (tx, rx) = mpsc::channel(1);

        let mut exex_notifications = self.notifications_exex.subscribe();
        tokio::spawn(async move {
            while let Ok(notification) = exex_notifications.recv().await {
                match TryInto::<ProtoExExNotification>::try_into(&notification) {
                    Ok(notification) => {
                        if let Err(e) = tx.send(Ok(notification)).await {
                            error!(error=?e , "exex.send");
                            break;
                        }
                    }
                    Err(e) => {
                        error!(error=?e , "ExExNotification::try_into");
                    }
                }
            }
            error!("exex notification loop finished");
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe_mempool_tx(
        &self,
        _request: Request<ProtoSubscribeRequest>,
    ) -> Result<Response<Self::SubscribeMempoolTxStream>, Status> {
        let (tx, rx) = mpsc::channel(1000);

        let mut notifications = self.notifications_tx.subscribe();
        tokio::spawn(async move {
            loop {
                match notifications.recv().await {
                    Ok(tx_signed) => match TryInto::<ProtoTransaction>::try_into(&tx_signed) {
                        Ok(transaction) => {
                            if let Err(e) = tx.send(Ok(transaction)).await {
                                error!(error=?e , "transaction.send");
                                break;
                            }
                        }
                        Err(e) => {
                            error!(error=?e , "Transaction::try_into");
                        }
                    },
                    Err(e) => {
                        error!(error=?e , "transaction.recv");
                    }
                }
            }
            error!("transaction loop finished");
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

async fn exex<Node>(mut ctx: ExExContext<Node>, notifications: broadcast::Sender<ExExNotification>) -> eyre::Result<()>
where
    Node: FullNodeComponents<Types: NodeTypes<Primitives = EthPrimitives>>,
{
    info!("ExEx worker started");

    while let Some(notification) = ctx.notifications.try_next().await? {
        let _ = notifications.send(notification.clone());

        if let Some(committed_chain) = notification.committed_chain() {
            if let Err(e) = ctx.events.send(ExExEvent::FinishedHeight(committed_chain.tip().num_hash())) {
                error!(error=?e, "ctx.events.send");
            }
        }
    }
    info!("ExEx worker finished");

    Ok(())
}

pub async fn mempool_worker<Pool>(mempool: Pool, notifications: broadcast::Sender<TransactionSigned>) -> eyre::Result<()>
where
    Pool: TransactionPool<Transaction = EthPooledTransaction> + Clone + 'static,
{
    info!("Mempool worker started");
    let mut tx_listener = mempool.new_transactions_listener();

    while let Some(tx_notification) = tx_listener.recv().await {
        let tx = tx_notification.transaction.to_consensus();
        let _ = notifications.send(tx.tx().clone());
    }
    info!("Mempool worker finished");

    Ok(())
}

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let notifications_exex = broadcast::channel(2).0;
        let notifications_tx = broadcast::channel(1000).0;

        let server = Server::builder()
            .add_service(RemoteExExServer::new(ExExService {
                notifications_exex: notifications_exex.clone(),
                notifications_tx: notifications_tx.clone(),
            }))
            .serve("[::1]:10000".parse().unwrap());

        let handle = builder
            .node(EthereumNode::default())
            .install_exex("Remote", |ctx| async move { Ok(exex(ctx, notifications_exex)) })
            .launch()
            .await?;

        let mempool = handle.node.pool.clone();

        tokio::task::spawn(mempool_worker(mempool, notifications_tx));

        handle.node.task_executor.spawn_critical("gRPC server", async move { server.await.expect("gRPC server crashed") });

        handle.wait_for_node_exit().await
    })
}
