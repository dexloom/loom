use eyre::{eyre, Result};
use loom_core_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_types_entities::{AccountNonceAndBalanceState, TxSigners};
use loom_types_events::{MessageTxCompose, TxCompose, TxComposeData};
use revm::DatabaseRef;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{debug, error, info};

/// encoder task performs encode for request
async fn router_task<DB: DatabaseRef + Send + Sync + Clone + 'static>(
    route_request: TxComposeData<DB>,
    compose_channel_tx: Broadcaster<MessageTxCompose<DB>>,
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
) -> Result<()> {
    debug!("Routing started {}", route_request.swap);

    let signer = match route_request.eoa {
        Some(eoa) => signers.read().await.get_signer_by_address(&eoa)?,
        None => signers.read().await.get_random_signer().ok_or(eyre!("NO_SIGNER"))?,
    };

    let nonce = account_monitor.read().await.get_account(&signer.address()).unwrap().get_nonce();
    let eth_balance = account_monitor.read().await.get_account(&signer.address()).unwrap().get_eth_balance();

    if route_request.next_block_base_fee == 0 {
        error!("Block base fee is not set");
        return Err(eyre!("NO_BLOCK_GAS_FEE"));
    }

    let gas = (route_request.swap.pre_estimate_gas()) * 2;

    let estimate_request = TxComposeData { signer: Some(signer), nonce, eth_balance, gas, ..route_request };
    let estimate_request = MessageTxCompose::estimate(estimate_request);

    match compose_channel_tx.send(estimate_request).await {
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
    compose_channel_rx: Broadcaster<MessageTxCompose<DB>>,
    compose_channel_tx: Broadcaster<MessageTxCompose<DB>>,
) -> WorkerResult {
    let mut compose_channel_rx: Receiver<MessageTxCompose<DB>> = compose_channel_rx.subscribe().await;

    info!("swap router worker started");

    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageTxCompose<DB>, RecvError> = msg;
                match msg {
                    Ok(compose_request) => {
                        if let TxCompose::Route(encode_request) = compose_request.inner {
                            debug!("MessageSwapPathEncodeRequest received. stuffing: {:?} swap: {}", encode_request.stuffing_txs_hashes, encode_request.swap);
                            tokio::task::spawn(
                                router_task(
                                    encode_request,
                                    compose_channel_tx.clone(),
                                    signers.clone(),
                                    account_monitor.clone(),
                                )
                            );
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
    compose_channel_rx: Option<Broadcaster<MessageTxCompose<DB>>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose<DB>>>,
}

impl<DB> SwapRouterActor<DB>
where
    DB: DatabaseRef + Send + Sync + Clone + Default + 'static,
{
    pub fn new() -> SwapRouterActor<DB> {
        SwapRouterActor { signers: None, account_nonce_balance: None, compose_channel_rx: None, compose_channel_tx: None }
    }

    pub fn with_signers(self, signers: SharedState<TxSigners>) -> Self {
        Self { signers: Some(signers), ..self }
    }

    pub fn on_bc(self, bc: &Blockchain<DB>) -> Self {
        Self {
            account_nonce_balance: Some(bc.nonce_and_balance()),
            compose_channel_rx: Some(bc.compose_channel()),
            compose_channel_tx: Some(bc.compose_channel()),
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
            self.compose_channel_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SwapRouterActor"
    }
}
