use alloy_primitives::Bytes;
use eyre::{eyre, Result};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tracing::{error, info};

use loom_core_actors::{Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Accessor, Consumer, Producer};
use loom_core_blockchain::Blockchain;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum, LoomTx};
use loom_types_events::{MessageTxCompose, RlpState, TxComposeData, TxComposeMessageType, TxState};

async fn sign_task<LDT: LoomDataTypes>(
    sign_request: TxComposeData<LDT>,
    compose_channel_tx: Broadcaster<MessageTxCompose<LDT>>,
) -> Result<()> {
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
            TxState::Stuffing(t) => RlpState::Stuffing(t.encode().into()),
            TxState::SignatureRequired(t) => {
                let tx = signer.sign_sync(t.clone()).unwrap();
                let tx_hash = tx.tx_hash();
                let signed_tx_bytes = Bytes::from(tx.encode());

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

    match compose_channel_tx.send(MessageTxCompose::broadcast(broadcast_request)) {
        Err(e) => {
            error!("{e}");
            Err(eyre!("BROADCAST_ERROR"))
        }
        _ => Ok(()),
    }
}

async fn request_listener_worker<LDT: LoomDataTypes>(
    compose_channel_rx: Broadcaster<MessageTxCompose<LDT>>,
    compose_channel_tx: Broadcaster<MessageTxCompose<LDT>>,
) -> WorkerResult {
    let mut compose_channel_rx: Receiver<MessageTxCompose<LDT>> = compose_channel_rx.subscribe();

    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let compose_request_msg : Result<MessageTxCompose<LDT>, RecvError> = msg;
                match compose_request_msg {
                    Ok(compose_request) =>{

                        if let TxComposeMessageType::Sign( sign_request)= compose_request.inner {
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

#[derive(Accessor, Consumer, Producer)]
pub struct TxSignersActor<LDT: LoomDataTypes + 'static = LoomDataTypesEthereum> {
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose<LDT>>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose<LDT>>>,
}

impl<LDT: LoomDataTypes + 'static> Default for TxSignersActor<LDT> {
    fn default() -> Self {
        Self { compose_channel_rx: None, compose_channel_tx: None }
    }
}

impl<LDT: LoomDataTypes> TxSignersActor<LDT> {
    pub fn new() -> TxSignersActor<LDT> {
        TxSignersActor::<LDT>::default()
    }

    pub fn on_bc(self, bc: &Blockchain<LDT>) -> Self {
        Self { compose_channel_rx: Some(bc.tx_compose_channel()), compose_channel_tx: Some(bc.tx_compose_channel()) }
    }
}

impl<LDT: LoomDataTypes> Actor for TxSignersActor<LDT> {
    fn start(&self) -> ActorResult {
        let task =
            tokio::task::spawn(request_listener_worker(self.compose_channel_rx.clone().unwrap(), self.compose_channel_tx.clone().unwrap()));

        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SignersActor"
    }
}
