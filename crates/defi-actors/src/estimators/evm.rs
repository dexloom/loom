use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Bytes, TxKind};
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use async_trait::async_trait;
use eyre::{eyre, Result};
use log::{error, info};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

use defi_entities::{GasStation, NWETH};
use defi_events::{MessageTxCompose, TxCompose, TxComposeData, TxState};
use loom_actors::{Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_actors_macros::{Consumer, Producer};
use loom_multicaller::SwapStepEncoder;
use loom_utils::evm::{env_for_block, evm_access_list};

use crate::estimators::tips::tips_and_value_for_swap_type;

async fn estimator_task(
    estimate_request: TxComposeData,
    encoder: Arc<SwapStepEncoder>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> Result<()> {
    info!("EVM estimation");

    let start_time = chrono::Local::now();


    let tx_signer = estimate_request.signer.clone().ok_or(eyre!("NO_SIGNER"))?;

    let opcodes = estimate_request.opcodes.clone().ok_or(eyre!("NO_OPCODES"))?;

    let profit = estimate_request.swap.abs_profit();
    if profit.is_zero() {
        return Err(eyre!("NO_PROFIT"));
    }

    let gas_price = estimate_request.priority_gas_fee + estimate_request.gas_fee;
    let gas_cost = GasStation::calc_gas_cost(100_000, gas_price);


    let (tips_vec, call_value) = tips_and_value_for_swap_type(&estimate_request.swap, None, gas_cost, estimate_request.eth_balance)?;

    let mut tips_opcodes = opcodes.clone();

    for tips in tips_vec {
        tips_opcodes = encoder.encode_tips(tips_opcodes, tips.token_in.get_address(), tips.min_change, tips.tips, tx_signer.address())?;
    }


    let (to, calldata) = encoder.to_call_data(&tips_opcodes)?;


    let tx_request = TransactionRequest {
        transaction_type: Some(2),
        chain_id: Some(1),
        from: Some(tx_signer.address()),
        to: Some(TxKind::Call(to)),
        gas: Some(estimate_request.gas),
        value: Some(call_value),
        input: TransactionInput::new(calldata.clone()),
        nonce: Some(estimate_request.nonce.into()),
        max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee),
        max_fee_per_gas: Some(estimate_request.gas_fee),
        ..TransactionRequest::default()
    };


    if let Some(db) = estimate_request.poststate {
        let evm_env = env_for_block(estimate_request.block, estimate_request.block_timestamp);
        match evm_access_list(&db, &evm_env, &tx_request) {
            Ok((gas_used, access_list)) => {
                let swap = estimate_request.swap.clone();

                let gas_cost = GasStation::calc_gas_cost(gas_used as u128, gas_price);

                if gas_cost > estimate_request.swap.abs_profit_eth() {
                    error!("Profit is too small");
                    return Err(eyre!("TOO_SMALL_PROFIT"));
                }

                let (tips_vec, call_value) = tips_and_value_for_swap_type(&estimate_request.swap, None, gas_cost, estimate_request.eth_balance)?;

                let call_value = if call_value.is_zero() { None } else { Some(call_value) };

                let mut tips_opcodes = opcodes.clone();

                for tips in tips_vec.iter() {
                    tips_opcodes = encoder.encode_tips(tips_opcodes, tips.token_in.get_address(), tips.min_change, tips.tips, tx_signer.address())?;
                }

                let (to, calldata) = encoder.to_call_data(&tips_opcodes)?;


                let tx_request = TransactionRequest {
                    transaction_type: Some(2),
                    chain_id: Some(1),
                    from: Some(tx_signer.address()),
                    to: Some(TxKind::Call(to)),
                    gas: Some((gas_used as u128 * 1200) / 1000),
                    value: call_value,
                    input: TransactionInput::new(calldata),
                    nonce: Some(estimate_request.nonce),
                    access_list: Some(access_list),
                    max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee),
                    max_fee_per_gas: Some(estimate_request.gas_fee),
                    ..TransactionRequest::default()
                };

                let encoded_txes: Result<Vec<TxEnvelope>, _> = estimate_request.stuffing_txs.iter().map(|item| TxEnvelope::try_from(item.clone())).collect();

                let stuffing_txs_rlp: Vec<Bytes> = encoded_txes?.into_iter().map(|x| Bytes::from(x.encoded_2718())).collect();


                let mut tx_with_state: Vec<TxState> = stuffing_txs_rlp.into_iter().map(|item| TxState::ReadyForBroadcastStuffing(item)).collect();

                tx_with_state.push(TxState::SignatureRequired(tx_request));

                let total_tips = tips_vec.into_iter().map(|v| v.tips).sum();
                let profit_eth = estimate_request.swap.abs_profit_eth();
                let gas_cost_f64 = NWETH::to_float(gas_cost);
                let tips_f64 = NWETH::to_float(total_tips);
                let profit_eth_f64 = NWETH::to_float(profit_eth);
                let profit_f64 = match estimate_request.swap.get_first_token() {
                    Some(token_in) => token_in.to_float(estimate_request.swap.abs_profit()),
                    None => profit_eth_f64
                };


                let sign_request = MessageTxCompose::sign(
                    TxComposeData {
                        tx_bundle: Some(tx_with_state),
                        poststate: Some(db),
                        tips: Some(total_tips + gas_cost),
                        ..estimate_request
                    }
                );

                let result = match compose_channel_tx.send(sign_request).await {
                    Err(e) => {
                        error!("{e}");
                        Err(eyre!("COMPOSE_CHANNEL_SEND_ERROR"))
                    }
                    _ => { Ok(()) }
                };

                let sim_duration = chrono::Local::now() - start_time;


                //TODO add formated paths
                info!(" +++ Simulation successful. Cost {} Profit {} ProfitEth {} Tips {} {} {}",  gas_cost_f64, profit_f64, profit_eth_f64, tips_f64, swap, sim_duration );

                result
            }
            Err(e) => {
                error!("evm_access_list error : {e}");
                Err(eyre!("EVM_ACCESS_LIST_ERROR"))
            }
        }
    } else {
        error!("StateDB is None");
        Err(eyre!("STATE_DB_IS_NONE"))
    }
}

async fn estimator_worker(
    encoder: Arc<SwapStepEncoder>,
    mut compose_channel_rx: Receiver<MessageTxCompose>,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> WorkerResult {
    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let compose_request_msg : Result<MessageTxCompose, RecvError> = msg;
                match compose_request_msg {
                    Ok(compose_request) =>{
                        match compose_request.inner {
                            TxCompose::Estimate(estimate_request) => {
                                tokio::task::spawn(
                                    estimator_task(
                                        estimate_request,
                                        encoder.clone(),
                                        compose_channel_tx.clone(),
                                    )
                                );

                                /*
                                _ = estimator_task(
                                        estimate_request,
                                        encoder.clone(),
                                        compose_channel_tx.clone(),
                                    ).await;

                                 */
                            }
                            _=>{

                            }
                        }
                    }
                    Err(e)=>{error!("{e}")}
                }
            }
        }
    }
}

#[derive(Consumer, Producer)]
pub struct EvmEstimatorActor
{
    encoder: Arc<SwapStepEncoder>,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl EvmEstimatorActor {
    pub fn new(encoder: Arc<SwapStepEncoder>) -> Self {
        Self {
            encoder,
            compose_channel_tx: None,
            compose_channel_rx: None,
        }
    }
}

#[async_trait]
impl Actor for EvmEstimatorActor {
    async fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(
            estimator_worker(
                self.encoder.clone(),
                self.compose_channel_rx.clone().unwrap().subscribe().await,
                self.compose_channel_tx.clone().unwrap(),
            )
        );
        Ok(vec![task])
    }
    fn name(&self) -> &'static str {
        "EvmEstimatorActor"
    }
}