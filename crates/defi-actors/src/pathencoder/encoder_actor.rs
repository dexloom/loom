use std::fmt::{Display, Formatter};

use alloy_primitives::{Address, U256};
use async_trait::async_trait;
use eyre::{eyre, OptionExt, Result};
use log::{debug, error, info};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_entities::{AccountNonceAndBalanceState, LatestBlock, SwapAmountType, SwapLine, SwapStep, TxSigners};
use defi_events::{MessageTxCompose, SwapType, TxCompose, TxComposeData};
use defi_types::{Mempool, Opcodes};
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_multicaller::SwapStepEncoder;

fn swap_type_to_swap_step(swap: &SwapType, multicaller: Address) -> Option<(SwapStep, SwapStep)> {
    match swap {
        SwapType::BackrunSwapLine(swap_line) => {
            let mut sp0: Option<SwapLine> = None;
            let mut sp1: Option<SwapLine> = None;

            for i in 1..swap_line.path.pool_count() {
                let (flash_path, inside_path) = swap_line.split(i).unwrap();
                if flash_path.can_flash_swap() || inside_path.can_flash_swap() {
                    sp0 = Some(flash_path);
                    sp1 = Some(inside_path);
                    break;
                }
            };

            if sp0.is_none() || sp1.is_none() {
                let (flash_path, inside_path) = swap_line.split(1).unwrap();
                sp0 = Some(flash_path);
                sp1 = Some(inside_path);
            }

            let mut step_0 = SwapStep::new(multicaller);
            step_0.add(sp0.unwrap());

            let mut step_1 = SwapStep::new(multicaller);
            let mut sp1 = sp1.unwrap();
            sp1.amount_in = SwapAmountType::Balance(multicaller);
            step_1.add(sp1);

            Some((step_0, step_1))
        }
        SwapType::BackrunSwapSteps((sp0, sp1)) => {
            Some((sp0.clone(), sp1.clone()))
        }
        _ => {
            None
        }
    }
}

fn encode_swap_steps(encoder: &SwapStepEncoder, sp0: &SwapStep, sp1: &SwapStep) -> Result<Opcodes> {
    if sp0.can_flash_swap() {
        encoder.encode_in_amount(sp0.clone(), sp1.clone())
    } else if sp1.can_flash_swap() {
        encoder.encode_out_amount(sp0.clone(), sp1.clone())
    } else {
        encoder.encode_balancer_flash_loan(vec![sp0.clone(), sp1.clone()])
    }
}

/// encoder task performs encode for request
async fn encoder_task(
    encode_request: TxComposeData,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
    encoder: SwapStepEncoder,
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
    latest_block: SharedState<LatestBlock>,
) -> Result<()> {
    info!("Encoding started {}", encode_request.swap);

    let swap_vec = match &encode_request.swap {
        SwapType::BackrunSwapLine(_) | SwapType::BackrunSwapSteps(_) => {
            vec![swap_type_to_swap_step(&encode_request.swap, encoder.get_multicaller()).ok_or_eyre("SWAP_TYPE_NOTE_COVERED")?]
        }
        SwapType::Multiple(swap_vec) => {
            let mut ret: Vec<(SwapStep, SwapStep)> = Vec::new();
            for s in swap_vec.iter() {
                ret.push(swap_type_to_swap_step(s, encoder.get_multicaller()).ok_or_eyre("AA")?);
            }
            ret
        }
        SwapType::None => {
            vec![]
        }
    };

    if swap_vec.len() == 0 {
        return Err(eyre!("NO_SWAP_STEPS"));
    }

    let swap_opcodes = if swap_vec.len() == 1 {
        let sp0 = &swap_vec[0].0;
        let sp1 = &swap_vec[0].1;
        encode_swap_steps(&encoder, sp0, sp1)?
    } else {
        let mut ret = Opcodes::new();
        for (sp0, sp1) in swap_vec.iter() {
            ret = encoder.encode_do_calls(ret, encode_swap_steps(&encoder, sp0, sp1)?)?;
        }
        ret
    };

    let signer = signers.read().await.get_randon_signer();

    let nonce = account_monitor.read().await.get_account(&signer.address()).unwrap().get_nonce();
    let eth_balance = account_monitor.read().await.get_account(&signer.address()).unwrap().get_eth_balance();


    let gas_fee: u128 = encode_request.gas_fee;

    if gas_fee == 0 {
        Err(eyre!("NO_BLOCK_GAS_FEE"))
    } else {
        let gas = 3_000_000;
        let value = U256::ZERO;
        let priority_gas_fee: u128 = 10_u128.pow(9);


        let estimate_request = TxComposeData {
            signer: Some(signer),
            nonce,
            eth_balance,
            gas,
            gas_fee,
            priority_gas_fee,
            value,
            opcodes: Some(swap_opcodes),
            ..encode_request
        };

        let estimate_request = MessageTxCompose::estimate(estimate_request);

        match compose_channel_tx.send(estimate_request).await {
            Err(e) => {
                error!("{e}");
                Err(eyre!(e))
            }
            Ok(_) => { Ok(()) }
        }
    }
}

async fn arb_swap_path_encoder_worker(
    encoder: SwapStepEncoder,
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
    latest_block: SharedState<LatestBlock>,
    mempool: SharedState<Mempool>,
    mut compose_channel_rx: Receiver<MessageTxCompose>,
    mut compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult
{
    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageTxCompose, RecvError> = msg;
                match msg {
                    Ok(compose_request) => {
                        match compose_request.inner {
                            TxCompose::Encode(encode_request) => {
                                info!("MessageSwapPathEncodeRequest received. stuffing: {:?} swap: {}", encode_request.stuffing_txs_hashes, encode_request.swap);
                                tokio::task::spawn(
                                    encoder_task(
                                        encode_request,
                                        compose_channel_tx.clone(),
                                        encoder.clone(),
                                        signers.clone(),
                                        account_monitor.clone(),
                                        latest_block.clone(),
                                        //mempool.clone(),
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


#[derive(Consumer, Producer, Accessor)]
pub struct ArbSwapPathEncoderActor
{
    encoder: SwapStepEncoder,
    #[accessor]
    mempool: Option<SharedState<Mempool>>,
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    #[accessor]
    account_monitor: Option<SharedState<AccountNonceAndBalanceState>>,
    #[accessor]
    latest_block: Option<SharedState<LatestBlock>>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl ArbSwapPathEncoderActor {
    pub fn new(multicaller: Address) -> ArbSwapPathEncoderActor {
        ArbSwapPathEncoderActor {
            encoder: SwapStepEncoder::new(multicaller),
            mempool: None,
            signers: None,
            account_monitor: None,
            latest_block: None,
            compose_channel_rx: None,
            compose_channel_tx: None,
        }
    }
}

#[async_trait]
impl Actor for ArbSwapPathEncoderActor {
    async fn start(&mut self) -> ActorResult {
        let task = tokio::task::spawn(
            arb_swap_path_encoder_worker(
                self.encoder.clone(),
                self.signers.clone().unwrap(),
                self.account_monitor.clone().unwrap(),
                self.latest_block.clone().unwrap(),
                self.mempool.clone().unwrap(),
                self.compose_channel_rx.clone().unwrap().subscribe().await,
                self.compose_channel_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }
}
