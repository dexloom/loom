use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Bytes, U256};
use alloy_provider::Provider;
use async_trait::async_trait;
use eyre::{eyre, Result};
use log::error;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_entities::LatestBlock;
use defi_events::{BestTxCompose, MessageTxCompose, RlpState, TxCompose, TxComposeData};
use flashbots::Flashbots;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

async fn broadcast_task<P>(
    broadcast_request: TxComposeData,
    client: Arc<Flashbots<P>>,
) -> Result<()>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    let block_number = broadcast_request.block;


    if let Some(rlp_bundle) = broadcast_request.rlp_bundle.clone() {
        let stuffing_rlp_bundle: Vec<Bytes> = rlp_bundle.iter().map(|item| item.unwrap()).collect();
        let backrun_rlp_bundle: Vec<Bytes> = rlp_bundle.iter().filter(|item| matches!(item, RlpState::Backrun(_))).map(|item| item.unwrap()).collect();

        if stuffing_rlp_bundle.iter().any(|i| i.is_empty()) || backrun_rlp_bundle.iter().any(|i| i.is_empty()) {
            Err(eyre!("RLP_BUNDLE_IS_INCORRECT"))
        } else {
            client.broadcast_txes(backrun_rlp_bundle.clone(), block_number).await?;
            client.broadcast_txes(stuffing_rlp_bundle.clone(), block_number).await?;

            tokio::time::sleep(Duration::from_millis(300)).await;
            client.broadcast_txes(stuffing_rlp_bundle, block_number + 1).await?;
            client.broadcast_txes(backrun_rlp_bundle.clone(), block_number + 1).await?;
            Ok(())
        }
    } else {
        error!("rlp_bundle is None");
        Err(eyre!("RLP_BUNDLE_IS_NONE"))
    }
}

async fn flashbots_broadcaster_worker<P>(
    client: Arc<Flashbots<P>>,
    smart_mode: bool,
    mut bundle_rx: Receiver<MessageTxCompose>,
) -> WorkerResult
where
    P: Provider + Send + Sync + Clone + 'static,
{
    let mut current_block: u64 = 0;
    let mut best_request: BestTxCompose = Default::default();

    loop {
        tokio::select! {
            msg = bundle_rx.recv() => {
                let broadcast_msg : Result<MessageTxCompose, RecvError> = msg;
                match broadcast_msg {
                    Ok(compose_request) => {
                        match compose_request.inner {
                            TxCompose::Broadcast(broadcast_request) => {
                                if smart_mode {
                                    if current_block < broadcast_request.block {
                                        current_block = broadcast_request.block;
                                        best_request = BestTxCompose::new_with_pct( U256::from(8000));
                                    }

                                    if best_request.check(&broadcast_request) {
                                        tokio::task::spawn(
                                            broadcast_task(
                                            broadcast_request,
                                            client.clone(),
                                            )
                                        );
                                    }


                                }else{
                                    tokio::task::spawn(
                                        broadcast_task(
                                            broadcast_request,
                                            client.clone(),
                                            //latest_block.clone(),
                                        )
                                    );
                                }


                            }
                            _=>{}
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
pub struct FlashbotsBroadcastActor<P>
{
    client: Arc<Flashbots<P>>,
    smart: bool,
    #[consumer]
    tx_compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
}

impl<P> FlashbotsBroadcastActor<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    pub fn new(client: Flashbots<P>, smart: bool) -> FlashbotsBroadcastActor<P> {
        FlashbotsBroadcastActor {
            client: Arc::new(client),
            smart,
            tx_compose_channel_rx: None,
        }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            tx_compose_channel_rx: Some(bc.compose_channel()),
            ..self
        }
    }
}

#[async_trait]
impl<P> Actor for FlashbotsBroadcastActor<P>
where
    P: Provider + Send + Sync + Clone + 'static,
{
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(
            flashbots_broadcaster_worker(
                self.client.clone(),
                self.smart,
                self.tx_compose_channel_rx.clone().unwrap().subscribe().await,
            )
        );
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "FlashbotsBroadcastActor"
    }
}


