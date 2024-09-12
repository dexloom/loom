use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Bytes, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use eyre::{eyre, Result};
use log::{error, info};
use tokio::sync::broadcast::error::RecvError;

use debug_provider::DebugProviderExt;
use defi_events::{MessageTxCompose, TxCompose, TxComposeData, TxState};
use loom_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_actors_macros::{Consumer, Producer};
use loom_multicaller::SwapStepEncoder;

async fn estimator_worker(
    encoder: Arc<SwapStepEncoder>,
    compose_channel_rx: Broadcaster<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult {
    subscribe!(compose_channel_rx);

    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let compose_request_msg : Result<MessageTxCompose, RecvError> = msg;
                match compose_request_msg {
                    Ok(compose_request) =>{
                        if let TxCompose::Estimate(estimate_request) = compose_request.inner {
                            info!("Hardhat estimation");
                            let token_in = estimate_request.swap.get_first_token().cloned().ok_or(eyre!("NO_TOKEN"))?;

                            let token_in_address = token_in.get_address();

                            let tx_signer = estimate_request.signer.clone().ok_or(eyre!("NO_SIGNER"))?;

                            let opcodes = estimate_request.opcodes.clone().ok_or(eyre!("NO_OPCODES"))?;

                            let profit = estimate_request.swap.abs_profit();
                            if profit.is_zero() {
                                return Err(eyre!("NO_PROFIT"))
                            }

                            let profit_eth = token_in.calc_eth_value( profit).ok_or(eyre!("CALC_ETH_VALUE_FAILED"))?;


                            let tips_opcodes = encoder.encode_tips( opcodes.clone(), token_in_address, profit >> 1, U256::from(1000), tx_signer.address() )?;

                            let (to, calldata) = encoder.to_call_data(&tips_opcodes)?;

                            let tx_request = TransactionRequest {
                                transaction_type : Some(2),
                                chain_id : Some(1),
                                from: Some(tx_signer.address()),
                                to: Some(TxKind::Call(to)),
                                gas: Some(estimate_request.gas),
                                value: Some(U256::from(1000)),
                                input: TransactionInput::new(calldata),
                                nonce: Some(estimate_request.nonce ),
                                max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee),
                                max_fee_per_gas: Some(estimate_request.gas_fee),
                                ..TransactionRequest::default()
                            };

                            let gas_price = estimate_request.priority_gas_fee + estimate_request.gas_fee;

                            if U256::from(300_000 * gas_price) > profit_eth {
                                error!("Profit is too small");
                                return Err(eyre!("TOO_SMALL_PROFIT"));
                            }

                            let enveloped_txs : Result<Vec<TxEnvelope>,_>= estimate_request.stuffing_txs.iter().map(|item| item.clone().try_into()).collect();
                            let stuffing_txs_rlp : Vec<Bytes> = enveloped_txs?.into_iter().map(|x| Bytes::from(x.encoded_2718()) ).collect();

                            let mut tx_with_state: Vec<TxState> = stuffing_txs_rlp.into_iter().map(TxState::ReadyForBroadcastStuffing).collect();

                            tx_with_state.push(TxState::SignatureRequired(tx_request));

                            let sign_request = MessageTxCompose::sign(
                                TxComposeData{
                                    tx_bundle : Some(tx_with_state),
                                    ..estimate_request
                                }
                            );

                            if let Err(e) = compose_channel_tx.send(sign_request).await {
                                error!("{e}");
                            }
                        }
                    }
                    Err(e)=>{error!("{e}")}
                }
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Consumer, Producer)]
pub struct HardhatEstimatorActor<P> {
    client: P,
    encoder: Arc<SwapStepEncoder>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl<P: Provider + DebugProviderExt + Send + Sync + Clone + 'static> HardhatEstimatorActor<P> {
    pub fn new(client: P, encoder: Arc<SwapStepEncoder>) -> Self {
        Self { client, encoder, compose_channel_tx: None, compose_channel_rx: None }
    }
}

impl<P: Provider + DebugProviderExt + Clone + Send + Sync + 'static> Actor for HardhatEstimatorActor<P> {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(estimator_worker(
            self.encoder.clone(),
            self.compose_channel_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "HardhatEstimatorActor"
    }
}
