use std::sync::Arc;
use std::time::Duration;

use alloy_network::Ethereum;
use alloy_primitives::{Bytes, U256};
use alloy_provider::Provider;
use alloy_transport::Transport;
use eyre::{eyre, Result};
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info};

use loom_broadcast_flashbots::Flashbots;
use loom_core_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer};
use loom_core_blockchain::Blockchain;
use loom_types_events::{BestTxCompose, MessageTxCompose, RlpState, TxCompose, TxComposeData};

async fn broadcast_task<P, T>(broadcast_request: TxComposeData, client: Arc<Flashbots<P, T>>) -> Result<()>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    let block_number = broadcast_request.next_block_number;

    if let Some(rlp_bundle) = broadcast_request.rlp_bundle.clone() {
        let stuffing_rlp_bundle: Vec<Bytes> = rlp_bundle.iter().map(|item| item.unwrap()).collect();
        let backrun_rlp_bundle: Vec<Bytes> =
            rlp_bundle.iter().filter(|item| matches!(item, RlpState::Backrun(_))).map(|item| item.unwrap()).collect();

        if stuffing_rlp_bundle.iter().any(|i| i.is_empty()) || backrun_rlp_bundle.iter().any(|i| i.is_empty()) {
            Err(eyre!("RLP_BUNDLE_IS_INCORRECT"))
        } else {
            client.broadcast_txes(backrun_rlp_bundle.clone(), block_number).await?;
            client.broadcast_txes(stuffing_rlp_bundle.clone(), block_number).await?;

            tokio::time::sleep(Duration::from_millis(300)).await;
            client.broadcast_txes(stuffing_rlp_bundle, block_number + 1).await?;
            client.broadcast_txes(backrun_rlp_bundle, block_number + 1).await?;
            Ok(())
        }
    } else {
        error!("rlp_bundle is None");
        Err(eyre!("RLP_BUNDLE_IS_NONE"))
    }
}

async fn flashbots_broadcaster_worker<P, T>(
    client: Arc<Flashbots<P, T>>,
    smart_mode: bool,
    bundle_rx: Broadcaster<MessageTxCompose>,
    allow_broadcast: bool,
) -> WorkerResult
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    subscribe!(bundle_rx);

    let mut current_block: u64 = 0;
    let mut best_request: BestTxCompose = Default::default();

    loop {
        tokio::select! {
            msg = bundle_rx.recv() => {
                let broadcast_msg : Result<MessageTxCompose, RecvError> = msg;
                match broadcast_msg {
                    Ok(compose_request) => {
                        if let TxCompose::Broadcast(broadcast_request)  = compose_request.inner {
                            if smart_mode {
                                if current_block < broadcast_request.next_block_number {
                                    current_block = broadcast_request.next_block_number;
                                    best_request = BestTxCompose::new_with_pct( U256::from(8000));
                                }

                                if best_request.check(&broadcast_request) {
                                    if allow_broadcast {
                                         tokio::task::spawn(
                                            broadcast_task(
                                            broadcast_request,
                                            client.clone(),
                                            )
                                        );
                                    } else {
                                       info!("broadcast_request (best_request): {:?}", broadcast_request);
                                    }
                                }
                            } else if allow_broadcast {
                                      tokio::task::spawn(
                                        broadcast_task(
                                            broadcast_request,
                                            client.clone(),
                                        )
                                    );
                            } else {
                                info!("broadcast_request: {:?}", broadcast_request);
                            }
                        }
                    }
                    Err(e)=>{
                        error!("flashbots_broadcaster_worker {}", e)
                    }
                }
            }
        }
    }
}

#[derive(Accessor, Consumer)]
pub struct FlashbotsBroadcastActor<P, T> {
    client: Arc<Flashbots<P, T>>,
    smart: bool,
    #[consumer]
    tx_compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    allow_broadcast: bool,
}

impl<P, T> FlashbotsBroadcastActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    pub fn new(client: Flashbots<P, T>, smart: bool, allow_broadcast: bool) -> FlashbotsBroadcastActor<P, T> {
        FlashbotsBroadcastActor { client: Arc::new(client), smart, tx_compose_channel_rx: None, allow_broadcast }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { tx_compose_channel_rx: Some(bc.compose_channel()), ..self }
    }
}

impl<P, T> Actor for FlashbotsBroadcastActor<P, T>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(flashbots_broadcaster_worker(
            self.client.clone(),
            self.smart,
            self.tx_compose_channel_rx.clone().unwrap(),
            self.allow_broadcast,
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "FlashbotsBroadcastActor"
    }
}
