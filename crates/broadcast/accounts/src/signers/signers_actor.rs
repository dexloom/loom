use alloy_consensus::TxEnvelope;
use alloy_rlp::Encodable;
use eyre::{eyre, Result};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{error, info};

use loom_core_actors::{Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_types_events::{MessageTxCompose, RlpState, TxCompose, TxComposeData, TxState};

async fn sign_task(sign_request: TxComposeData, compose_channel_tx: Broadcaster<MessageTxCompose>) -> Result<()> {
    let signer = match sign_request.signer.clone() {
        Some(signer) => signer,
        None => {
            error!("No signer found in sign_request");
            return Err(eyre!("NO_SIGNER_FOUND"));
        }
    };

    let rlp_bundle: Vec<RlpState> = sign_request
        .tx_bundle
        .clone()
        .unwrap()
        .iter()
        .map(|tx_request| match &tx_request {
            TxState::Stuffing(t) => {
                let typed_tx: Result<TxEnvelope, _> = t.clone().try_into();

                match typed_tx {
                    Ok(typed_tx) => {
                        let mut v: Vec<u8> = Vec::new();
                        typed_tx.encode(&mut v);
                        RlpState::Stuffing(v.into())
                    }
                    _ => RlpState::None,
                }
            }
            TxState::SignatureRequired(t) => {
                let (tx_hash, signed_tx_bytes) = signer.sign_sync(t.clone()).unwrap();
                info!("Tx signed {tx_hash:?}");
                RlpState::Backrun(signed_tx_bytes)
            }
            TxState::ReadyForBroadcast(t) => RlpState::Backrun(t.clone()),
            TxState::ReadyForBroadcastStuffing(t) => RlpState::Stuffing(t.clone()),
        })
        .collect();

    if rlp_bundle.iter().any(|item| item.is_none()) {
        error!("Bundle is not ready. Cannot sign");
        return Err(eyre!("CANNOT_SIGN_BUNDLE"));
    }

    let broadcast_request = TxComposeData { rlp_bundle: Some(rlp_bundle), ..sign_request };

    match compose_channel_tx.send(MessageTxCompose::broadcast(broadcast_request)).await {
        Err(e) => {
            error!("{e}");
            Err(eyre!("BROADCAST_ERROR"))
        }
        _ => Ok(()),
    }
}

async fn request_listener_worker(
    compose_channel_rx: Broadcaster<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult {
    let mut compose_channel_rx: Receiver<MessageTxCompose> = compose_channel_rx.subscribe().await;

    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let compose_request_msg : Result<MessageTxCompose, RecvError> = msg;
                match compose_request_msg {
                    Ok(compose_request) =>{

                        if let TxCompose::Sign( sign_request)= compose_request.inner {
                            tokio::task::spawn(
                                sign_task(
                                    sign_request,
                                    compose_channel_tx.clone(),
                                )
                            );
                        }
                    }
                    Err(e)=>{error!("{}",e)}
                }
            }
        }
    }
}

#[derive(Accessor, Consumer, Producer, Default)]
pub struct TxSignersActor {
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl TxSignersActor {
    pub fn new() -> TxSignersActor {
        TxSignersActor::default()
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { compose_channel_rx: Some(bc.compose_channel()), compose_channel_tx: Some(bc.compose_channel()) }
    }
}

impl Actor for TxSignersActor {
    fn start(&self) -> ActorResult {
        let task =
            tokio::task::spawn(request_listener_worker(self.compose_channel_rx.clone().unwrap(), self.compose_channel_tx.clone().unwrap()));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SignersActor"
    }
}
