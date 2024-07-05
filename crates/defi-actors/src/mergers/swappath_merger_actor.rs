use std::sync::Arc;

use alloy_primitives::{Address, U256};
use async_trait::async_trait;
use eyre::{eyre, Result};
use log::{debug, error, info};
use revm::InMemoryDB;
use revm::primitives::Env;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_entities::{AccountNonceAndBalanceState, LatestBlock, MarketState, Swap, SwapStep, TxSigners};
use defi_events::{MarketEvents, MessageTxCompose, TxCompose, TxComposeData};
use defi_types::Mempool;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_multicaller::SwapStepEncoder;

async fn arb_swap_steps_optimizer_task(
    compose_channel_tx: Broadcaster<MessageTxCompose>,
    state_db: Arc<InMemoryDB>,
    evm_env: Env,
    request: TxComposeData,
) -> Result<()>
{
    info!("Step Simulation started");

    if let Swap::BackrunSwapSteps((sp0, sp1)) = request.swap {
        let start_time = chrono::Local::now();
        match SwapStep::optimize_swap_steps(&state_db, evm_env, &sp0, &sp1, None) {
            Ok((s0, s1)) => {
                let encode_request = MessageTxCompose::encode(
                    TxComposeData {
                        origin: Some("merger_searcher".to_string()),
                        tips_pct: None,
                        swap: Swap::BackrunSwapSteps((s0, s1)),
                        ..request
                    }
                );
                compose_channel_tx.send(encode_request).await?;
            }
            Err(e) => {
                error!("Optimization error:{}",e);
                return Err(eyre!("OPTIMIZATION_ERROR"));
            }
        }
        info!("Step Optimization finished {} + {} {}", &sp0, &sp1, chrono::Local::now() - start_time);
    } else {
        error!("Incorrect swap_type");
        return Err(eyre!("INCORRECT_SWAP_TYPE"));
    }

    Ok(())
}


async fn arb_swap_path_merger_worker(
    encoder: SwapStepEncoder,
    //signers: SharedState<TxSigners>,
    //account_monitor: SharedState<AccountNonceAndBalanceState>,
    latest_block: SharedState<LatestBlock>,
    //mempool: SharedState<Mempool>,
    //market_state: SharedState<MarketState>,
    mut market_events_rx: Receiver<MarketEvents>,
    mut compose_channel_rx: Receiver<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult
{
    let mut ready_requests: Vec<TxComposeData> = Vec::new();
    //let mut state_db : InMemoryDB;

    //state_db = market_state.read().await.state_db.clone();

    loop {
        tokio::select! {
            msg = market_events_rx.recv() => {
                let msg : Result<MarketEvents, RecvError> = msg;
                match msg {
                    Ok(event) => {
                        match event {
                            MarketEvents::BlockHeaderUpdate{..} =>{
                                debug!("Cleaning ready requests");
                                ready_requests = Vec::new();
                            }
                            MarketEvents::BlockStateUpdate{..}=>{
                                debug!("State updated");
                                //state_db = market_state.read().await.state_db.clone();
                            }
                            _=>{}
                        }
                    }
                    Err(e)=>{error!("{}", e)}
                }

            },
            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageTxCompose, RecvError> = msg;
                match msg {
                    Ok(swap) => {

                        let compose_data = match swap.inner() {
                            TxCompose::Sign(data) => data,
                            _=>continue,
                        };

                        let swap_path = match &compose_data.swap {
                            Swap::BackrunSwapLine(path) => path,
                            _=>continue,
                        };


                        info!("MessageSwapPathEncodeRequest received. stuffing: {:?} swap: {}", compose_data.stuffing_txs_hashes, compose_data.swap);

                        for req in ready_requests.iter() {

                            let req_swap = match &req.swap {
                                Swap::BackrunSwapLine(path)=>path,
                                _ => continue,
                            };

                            // todo!() mega bundle merge
                            if !compose_data.same_stuffing(&req.stuffing_txs_hashes) {
                                continue
                            };


                            match SwapStep::merge_swap_paths( req_swap.clone(), swap_path.clone(), encoder.get_multicaller() ){
                                Ok((sp0, sp1)) => {
                                    let latest_block_guard = latest_block.read().await;
                                    let block_header = latest_block_guard.block_header.clone().unwrap();
                                    drop(latest_block_guard);

                                    let request = TxComposeData{
                                        swap : Swap::BackrunSwapSteps((sp0,sp1)),
                                        ..compose_data.clone()
                                    };

                                    let mut evm_env = Env::default();


                                    evm_env.block.number = U256::from(block_header.number.unwrap() + 1).into();
                                    let timestamp = block_header.timestamp;
                                    evm_env.block.timestamp = U256::from(timestamp +12);



                                    if let Some(db) = compose_data.poststate.clone() {
                                        tokio::task::spawn(
                                            arb_swap_steps_optimizer_task(
                                                //encoder.clone(),
                                                compose_channel_tx.clone(),
                                                db,
                                                evm_env,
                                                request
                                            )
                                        );
                                    }

                                    break; // only first

                                }
                                Err(e)=>{
                                    error!("SwapPath merge error : {} {}", ready_requests.len(), e);
                                }
                            }
                        }
                        ready_requests.push(compose_data.clone());
                        ready_requests.sort_by(|r0,r1| r1.swap.abs_profit().cmp(&r0.swap.abs_profit())  )

                    }
                    Err(e)=>{error!("{}",e)}
                }

            }
        }
    }
}


#[derive(Consumer, Producer, Accessor)]
pub struct ArbSwapPathMergerActor
{
    encoder: SwapStepEncoder,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    market_state: Option<SharedState<MarketState>>,
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    #[accessor]
    account_monitor: Option<SharedState<AccountNonceAndBalanceState>>,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    market_events: Option<Broadcaster<MarketEvents>>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl ArbSwapPathMergerActor
{
    pub fn new(multicaller: Address) -> ArbSwapPathMergerActor {
        ArbSwapPathMergerActor {
            encoder: SwapStepEncoder::new(multicaller),
            mempool: None,
            market_state: None,
            signers: None,
            account_monitor: None,
            latest_block: None,
            market_events: None,
            compose_channel_rx: None,
            compose_channel_tx: None,
        }
    }
}

#[async_trait]
impl Actor for ArbSwapPathMergerActor
{
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(
            arb_swap_path_merger_worker(
                //self.client.clone(),
                self.encoder.clone(),
                //self.signers.clone().unwrap(),
                //self.account_monitor.clone().unwrap(),
                self.latest_block.clone().unwrap(),
                //self.mempool.clone().unwrap(),
                //self.market_state.clone().unwrap(),
                self.market_events.clone().unwrap().subscribe().await,
                self.compose_channel_rx.clone().unwrap().subscribe().await,
                self.compose_channel_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "ArbSwapPathMergerActor"
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::{Address, U256};

    use defi_entities::{SwapAmountType, SwapLine, Token};
    use defi_events::{Swap, TxComposeData};

    #[test]
    pub fn test_sort() {
        let mut ready_requests: Vec<TxComposeData> = Vec::new();

        let mut sp0 = SwapLine::new();
        let mut sp1 = SwapLine::new();
        let token = Token::new("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>().unwrap());
        sp0.amount_in = SwapAmountType::Set(U256::from(1));
        sp0.amount_out = SwapAmountType::Set(U256::from(2));
        sp1.amount_in = SwapAmountType::Set(U256::from(10));
        sp1.amount_out = SwapAmountType::Set(U256::from(20));
        //sp0.tokens = vec![token.clone(), token.clone()];
        //sp1.tokens = vec![token.clone(), token.clone()];


        let r0 = TxComposeData {
            swap: Swap::BackrunSwapLine(sp0),
            ..TxComposeData::default()
        };
        let r1 = TxComposeData {
            swap: Swap::BackrunSwapLine(sp1),
            ..TxComposeData::default()
        };

        ready_requests.push(r0);
        ready_requests.push(r1);


        ready_requests.sort_by(|r0, r1| r1.swap.abs_profit().cmp(&r0.swap.abs_profit()));
        for r in ready_requests.iter() {
            println!("{:?}", r.swap);
        }
    }
}
