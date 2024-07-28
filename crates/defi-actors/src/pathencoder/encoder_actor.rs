use alloy_primitives::{Address, U256};
use async_trait::async_trait;
use eyre::{eyre, OptionExt, Result};
use log::{debug, error};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_blockchain::Blockchain;
use defi_entities::{AccountNonceAndBalanceState, Swap, SwapStep, TxSigners};
use defi_events::{MessageTxCompose, TxCompose, TxComposeData};
use defi_types::MulticallerCalls;
use loom_actors::{Accessor, Actor, ActorResult, Broadcaster, Consumer, Producer, SharedState, WorkerResult};
use loom_actors_macros::{Accessor, Consumer, Producer};
use loom_multicaller::SwapStepEncoder;

/// encoder task performs encode for request
async fn encoder_task(
    encode_request: TxComposeData,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
    encoder: SwapStepEncoder,
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
) -> Result<()> {
    debug!("Encoding started {}", encode_request.swap);

    let swap_vec = match &encode_request.swap {
        Swap::BackrunSwapLine(_) | Swap::BackrunSwapSteps(_) => {
            vec![encode_request.swap.to_swap_steps(encoder.get_multicaller()).ok_or_eyre("SWAP_TYPE_NOTE_COVERED")?]
        }
        Swap::Multiple(swap_vec) => {
            let mut ret: Vec<(SwapStep, SwapStep)> = Vec::new();
            for s in swap_vec.iter() {
                ret.push(s.to_swap_steps(encoder.get_multicaller()).ok_or_eyre("AA")?);
            }
            ret
        }
        Swap::ExchangeSwapLine(_) => vec![],
        Swap::None => {
            vec![]
        }
    };

    let swap_opcodes = if swap_vec.is_empty() {
        match &encode_request.swap {
            Swap::ExchangeSwapLine(swap_line) => {
                debug!("Swap::ExchangeSwapLine encoding started");
                match encoder.swap_line_encoder.encode_swap_line_in_amount(swap_line, encoder.multicaller, encoder.multicaller) {
                    Ok(calls) => calls,
                    Err(e) => {
                        error!("swap_line_encoder.encode_swap_line_in_amount : {}", e);
                        return Err(eyre!("ENCODING_FAILED"));
                    }
                }
            }
            _ => return Err(eyre!("NO_SWAP_STEPS")),
        }
    } else if swap_vec.len() == 1 {
        let sp0 = &swap_vec[0].0;
        let sp1 = &swap_vec[0].1;
        encoder.encode(sp0, sp1)?
    } else {
        let mut ret = MulticallerCalls::new();
        for (sp0, sp1) in swap_vec.iter() {
            ret = encoder.encode_do_calls(ret, encoder.encode(sp0, sp1)?)?;
        }
        ret
    };

    let signer = signers.read().await.get_randon_signer();
    match signer {
        Some(signer) => {
            let nonce = account_monitor.read().await.get_account(&signer.address()).unwrap().get_nonce();
            let eth_balance = account_monitor.read().await.get_account(&signer.address()).unwrap().get_eth_balance();

            let gas_fee: u128 = encode_request.gas_fee;

            if gas_fee == 0 {
                error!("Block base fee is not set");
                Err(eyre!("NO_BLOCK_GAS_FEE"))
            } else {
                let gas = (encode_request.swap.pre_estimate_gas() as u128) * 2;
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
                    Ok(_) => Ok(()),
                }
            }
        }
        None => Err(eyre!("NO_SIGNER_AVAILABLE")),
    }
}

async fn arb_swap_path_encoder_worker(
    encoder: SwapStepEncoder,
    signers: SharedState<TxSigners>,
    account_monitor: SharedState<AccountNonceAndBalanceState>,
    compose_channel_rx: Broadcaster<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult {
    let mut compose_channel_rx: Receiver<MessageTxCompose> = compose_channel_rx.subscribe().await;

    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let msg : Result<MessageTxCompose, RecvError> = msg;
                match msg {
                    Ok(compose_request) => {
                        if let TxCompose::Encode(encode_request) = compose_request.inner {
                            debug!("MessageSwapPathEncodeRequest received. stuffing: {:?} swap: {}", encode_request.stuffing_txs_hashes, encode_request.swap);
                            tokio::task::spawn(
                                encoder_task(
                                    encode_request,
                                    compose_channel_tx.clone(),
                                    encoder.clone(),
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

#[derive(Consumer, Producer, Accessor)]
pub struct SwapEncoderActor {
    encoder: SwapStepEncoder,
    #[accessor]
    signers: Option<SharedState<TxSigners>>,
    #[accessor]
    account_nonce_balance: Option<SharedState<AccountNonceAndBalanceState>>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl SwapEncoderActor {
    pub fn new(multicaller: Address) -> SwapEncoderActor {
        SwapEncoderActor {
            encoder: SwapStepEncoder::new(multicaller),
            signers: None,
            account_nonce_balance: None,
            compose_channel_rx: None,
            compose_channel_tx: None,
        }
    }

    pub fn with_signers(self, signers: SharedState<TxSigners>) -> Self {
        Self { signers: Some(signers), ..self }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self {
            account_nonce_balance: Some(bc.nonce_and_balance()),
            compose_channel_rx: Some(bc.compose_channel()),
            compose_channel_tx: Some(bc.compose_channel()),
            ..self
        }
    }
}

#[async_trait]
impl Actor for SwapEncoderActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(arb_swap_path_encoder_worker(
            self.encoder.clone(),
            self.signers.clone().unwrap(),
            self.account_nonce_balance.clone().unwrap(),
            self.compose_channel_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "ArbSwapPathEncoderActor"
    }
}
