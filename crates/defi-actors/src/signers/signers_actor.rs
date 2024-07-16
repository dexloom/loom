use alloy_consensus::TxEnvelope;
use alloy_rlp::Encodable;
use async_trait::async_trait;
use eyre::{eyre, Result};
use log::{error, info};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_events::{MessageTxCompose, RlpState, TxCompose, TxComposeData, TxState};
use loom_actors::{Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};

async fn sign_task(
    sign_request: TxComposeData,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> Result<()> {
    let signer = sign_request.signer.clone().unwrap();

    let rlp_bundle: Vec<RlpState> = sign_request.tx_bundle.clone().unwrap().iter().map(|tx_request| {
        match &tx_request {
            TxState::Stuffing(t) => {
                let typed_tx: Result<TxEnvelope, _> = t.clone().try_into();

                match typed_tx {
                    Ok(typed_tx) => {
                        let mut v: Vec<u8> = Vec::new();
                        typed_tx.encode(&mut v);
                        RlpState::Stuffing(v.into())
                    }
                    _ => {
                        RlpState::None
                    }
                }
            }
            TxState::SignatureRequired(t) => {
                let (tx_hash, signed_tx_bytes) = signer.sign_sync(t.clone()).unwrap();
                info!("Tx signed {tx_hash:?}");
                RlpState::Backrun(signed_tx_bytes)
            }
            TxState::ReadyForBroadcast(t) => {
                RlpState::Backrun(t.clone())
            }
            TxState::ReadyForBroadcastStuffing(t) => {
                RlpState::Stuffing(t.clone())
            }
        }
    }).collect();

    if rlp_bundle.iter().any(|item| item.is_none()) {
        error!("Bundle is not ready. Cannot sign");
        return Err(eyre!("CANNOT_SIGN_BUNDLE"));
    }
    //let rlp_bundle= rlp_bundle.into_iter().map(|item| item.unwrap()).collect();

    let broadcast_request = TxComposeData {
        rlp_bundle: Some(rlp_bundle),
        ..sign_request
    };


    match compose_channel_tx.send(
        MessageTxCompose::broadcast(broadcast_request),
    ).await {
        Err(e) => {
            error!("{e}");
            Err(eyre!("BROADCAST_ERROR"))
        }
        _ => { Ok(()) }
    }
}

async fn request_listener_worker(
    mut compose_channel_rx: Receiver<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>)
    -> WorkerResult
{
    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let compose_request_msg : Result<MessageTxCompose, RecvError> = msg;
                match compose_request_msg {
                    Ok(compose_request) =>{

                        match compose_request.inner {
                            TxCompose::Sign( sign_request)=>{
                                //let rlp_bundle : Vec<Option<Bytes>> = Vec::new();
                                tokio::task::spawn(
                                    sign_task(
                                        sign_request,
                                        compose_channel_tx.clone(),
                                    )
                                );
                            },
                            _=>{}
                        }

                    }
                    Err(e)=>{error!("{}",e)}
                }
            }
        }
    }
}


#[derive(Accessor, Consumer, Producer)]
pub struct TxSignersActor
{
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,

}

impl TxSignersActor
{
    pub fn new() -> TxSignersActor {
        TxSignersActor {
            compose_channel_rx: None,
            compose_channel_tx: None,
        }
    }


    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            compose_channel_rx: Some(bc.compose_channel()),
            compose_channel_tx: Some(bc.compose_channel()),
            ..self
        }
    }
}


#[async_trait]
impl Actor for TxSignersActor
{
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(
            request_listener_worker(
                self.compose_channel_rx.clone().unwrap().subscribe().await,
                self.compose_channel_tx.clone().unwrap(),
            )
        );


        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "SignersActor"
    }
}