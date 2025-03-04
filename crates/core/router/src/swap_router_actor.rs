use eyre::{eyre, Result};
use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::{Blockchain, Strategy};
use loom_types_entities::{AccountNonceAndBalanceState, TxSigners};
use loom_types_events::{MessageSwapCompose, MessageTxCompose, SwapComposeData, SwapComposeMessage, TxComposeData};
use revm::DatabaseRef;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{debug, error, info};

/// encoder task performs initial routing for swap request
async fn router_task_prepare<DB: DatabaseRef + Send + Sync + Clone + 'static>(
    route_request: SwapComposeData<DB>,
    compose_channel_tx: Broadcaster<MessageSwapCompose<DB>>,
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
) -> Result<()> {
    debug!("router_task_prepare started {}", route_request.swap);

    let signer = match route_request.tx_compose.eoa {
        Some(eoa) => signers.read().await.get_signer_by_address(&eoa)?,
        None => signers.read().await.get_random_signer().ok_or(eyre!("NO_SIGNER"))?,
    };

    let nonce = account_monitor.read().await.get_account(&signer.address()).unwrap().get_nonce();
    let eth_balance = account_monitor.read().await.get_account(&signer.address()).unwrap().get_eth_balance();

    if route_request.tx_compose.next_block_base_fee == 0 {
        error!("Block base fee is not set");
        return Err(eyre!("NO_BLOCK_GAS_FEE"));
    }

    let gas = (route_request.swap.pre_estimate_gas()) * 2;

    let estimate_request = SwapComposeData {
        tx_compose: TxComposeData { signer: Some(signer), nonce, eth_balance, gas, ..route_request.tx_compose },
        ..route_request
    };
    let estimate_request = MessageSwapCompose::estimate(estimate_request);

    match compose_channel_tx.send(estimate_request) {
        Err(_) => {
            error!("compose_channel_tx.send(estimate_request)");
            Err(eyre!("ERROR_SENDING_REQUEST"))
        }
        Ok(_) => Ok(()),
    }
}

async fn router_task_broadcast<DB: DatabaseRef + Send + Sync + Clone + 'static>(
    route_request: SwapComposeData<DB>,
    tx_compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> Result<()> {
    debug!("router_task_broadcast started {}", route_request.swap);

    let tx_compose = TxComposeData { swap: Some(route_request.swap), tips: route_request.tips, ..route_request.tx_compose };

    match tx_compose_channel_tx.send(MessageTxCompose::sign(tx_compose)) {
        Err(_) => {
            error!("compose_channel_tx.send(estimate_request)");
            Err(eyre!("ERROR_SENDING_REQUEST"))
        }
        Ok(_) => Ok(()),
    }
}

async fn swap_router_worker<DB: DatabaseRef + Clone + Send + Sync + 'static>(
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
    swap_compose_channel_rx: Broadcaster<MessageSwapCompose<DB>>,
    swap_compose_channel_tx: Broadcaster<MessageSwapCompose<DB>>,
    tx_compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult {
    let mut compose_channel_rx: Receiver<MessageSwapCompose<DB>> = swap_compose_channel_rx.subscribe();

    info!("swap router worker started");

    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageSwapCompose<DB>, RecvError> = msg;
                match msg {
                    Ok(compose_request) => {
                        match compose_request.inner {
                            SwapComposeMessage::Prepare(swap_compose_request)=>{
                                debug!("MessageSwapComposeRequest::Prepare received. stuffing: {:?} swap: {}", swap_compose_request.tx_compose.stuffing_txs_hashes, swap_compose_request.swap);
                                tokio::task::spawn(
                                    router_task_prepare(
                                        swap_compose_request,
                                        swap_compose_channel_tx.clone(),
                                        signers.clone(),
                                        account_monitor.clone(),
                                    )
                                );
                            }
                            SwapComposeMessage::Ready(swap_compose_request)=>{
                                debug!("MessageSwapComposeRequest::Ready received. stuffing: {:?} swap: {}", swap_compose_request.tx_compose.stuffing_txs_hashes, swap_compose_request.swap);
                                tokio::task::spawn(
                                    router_task_broadcast(
                                        swap_compose_request,
                                        tx_compose_channel_tx.clone(),
                                    )
                                );
                            }
                            _=>{}

                        }
                    }
                    Err(e)=>{error!("compose_channel_rx {}",e)}
                }
            }
        }
    }
}

#[derive(Consumer, Producer, Accessor, Default)]
pub struct SwapRouterActor<DB: Send + Sync + Clone + 'static> {
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    #[accessor]
    account_nonce_balance: Option<SharedState<AccountNonceAndBalanceState>>,
    #[consumer]
    swap_compose_channel_rx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    #[producer]
    swap_compose_channel_tx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    #[producer]
    tx_compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl<DB> SwapRouterActor<DB>
where
    DB: DatabaseRef + Send + Sync + Clone + Default + 'static,
{
    pub fn new() -> SwapRouterActor<DB> {
        SwapRouterActor {
            signers: None,
            account_nonce_balance: None,
            swap_compose_channel_rx: None,
            swap_compose_channel_tx: None,
            tx_compose_channel_tx: None,
        }
    }

    pub fn with_signers(self, signers: SharedState<TxSigners>) -> Self {
        Self { signers: Some(signers), ..self }
    }

    pub fn on_bc(self, bc: &Blockchain, strategy: &Strategy<DB>) -> Self {
        Self {
            swap_compose_channel_rx: Some(strategy.swap_compose_channel()),
            swap_compose_channel_tx: Some(strategy.swap_compose_channel()),
            account_nonce_balance: Some(bc.nonce_and_balance()),
            tx_compose_channel_tx: Some(bc.tx_compose_channel()),
            ..self
        }
    }
}

impl<DB> Actor for SwapRouterActor<DB>
where
    DB: DatabaseRef + Send + Sync + Clone + Default + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(swap_router_worker(
            self.signers.clone().unwrap(),
            self.account_nonce_balance.clone().unwrap(),
            self.swap_compose_channel_rx.clone().unwrap(),
            self.swap_compose_channel_tx.clone().unwrap(),
            self.tx_compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SwapRouterActor"
    }
}
