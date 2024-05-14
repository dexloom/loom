use alloy_provider::Provider;
use async_trait::async_trait;
use eyre::Result;
use log::{error, info};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_entities::LatestBlock;
use defi_events::{MessageTxCompose, TxCompose, TxComposeData};
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer};

async fn broadcast_task<P>(
    client: P,
    request: TxComposeData,
) -> Result<()>
    where
        P: Provider + Clone + Send + Sync + 'static
{
    info!("Hardhat broadcast request received : {}", request.origin.unwrap_or("UNKNOWN_ORIGIN".to_string()));
    //let snap = client.dev_rpc().snapshot().await?;
    //info!("Hardhat snapshot created {snap}");

    for tx_rlp in request.rlp_bundle.unwrap_or_default().iter() {
        match client.send_raw_transaction(tx_rlp.clone().unwrap().as_ref()).await {
            Err(e) => error!("send_raw_transaction error : {e}"),
            Ok(_) => {
                info!("Hardhat transaction broadcast successfully",);
                //TODO : Fix rlp decode
                /*
                let tx_bytes = tx_rlp.clone().unwrap();
                let rlp = Rlp::new(&tx_bytes);
                let tx = Transaction::decode(&rlp)?;
                for i in 0..10 {
                    match client.get_transaction_receipt(tx.hash).await {
                        Ok(receipt) => {
                            match receipt {
                                Some(receipt) => {
                                    let status = receipt.status.unwrap_or_default();

                                    if status.as_u64() == 1 {
                                        info!("Hardhat tx receipt success {:?} gas used {} status {}", receipt.transaction_hash, receipt.gas_used.unwrap_or_default(), status);
                                    } else {
                                        error!("Hardhat tx receipt error {:?} gas used {} status {}", receipt.transaction_hash, receipt.gas_used.unwrap_or_default(), status);
                                    }
                                    break;
                                }
                                None => tokio::time::sleep(Duration::from_millis(200)).await,
                            }
                        }
                        Err(e) => tokio::time::sleep(Duration::from_millis(200)).await,
                    }
                }
                 */
            }
        }
    }

    /*
    match client.dev_rpc().revert_to_snapshot(snap).await {
        Ok(_) => { info!("Hardhat reverted to snapshot {snap} successfully") }
        Err(e) => { error!("Error reverting to snapshot : {e}") }
    }

     */

    Ok(())
}

async fn hardhat_broadcaster_worker<P>(
    client: P,
    //latest_block: SharedState<LatestBlock>,
    mut bundle_rx: Receiver<MessageTxCompose>,
) -> WorkerResult
    where
        P: Provider + Send + Sync + Clone + 'static
{
    loop {
        tokio::select! {
            msg = bundle_rx.recv() => {
                let broadcast_msg : Result<MessageTxCompose, RecvError> = msg;
                match broadcast_msg {
                    Ok(compose_request) => {
                        match compose_request.inner {
                            TxCompose::Broadcast(broadcast_request) => {
                                info!("Broadcasting to hardhat:" );
                                match broadcast_task(client.clone(), broadcast_request).await{
                                    Err(e)=>error!("{e}"),
                                    Ok(_)=>info!("Hardhat broadcast successful")
                                }
                            }
                            _=>{}
                        }
                    }
                    Err(e)=>{
                        error!("{}", e)
                    }
                }
            }
        }
    }
}

#[derive(Accessor, Consumer)]
pub struct HardhatBroadcastActor<P>
{
    client: P,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    broadcast_rx: Option<Broadcaster<MessageTxCompose>>,
}

impl<P> HardhatBroadcastActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static
{
    pub fn new(client: P) -> HardhatBroadcastActor<P> {
        Self {
            client,
            latest_block: None,
            broadcast_rx: None,
        }
    }
}

#[async_trait]
impl<P> Actor for HardhatBroadcastActor<P>
    where
        P: Provider + Send + Sync + Clone + 'static
{
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            hardhat_broadcaster_worker(
                self.client.clone(),
                //self.latest_block.clone().unwrap(),
                self.broadcast_rx.clone().unwrap().subscribe().await,
            )
        );
        Ok(vec![task])
    }
}