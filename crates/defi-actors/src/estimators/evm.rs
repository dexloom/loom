use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Bytes, TxKind};
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use async_trait::async_trait;
use eyre::{eyre, Result};
use log::{debug, error, info};
use reth_primitives::U256;
use tokio::sync::broadcast::error::RecvError;

use defi_blockchain::Blockchain;
use defi_entities::{GasStation, Swap};
use loom_utils::NWETH;

use defi_events::{MessageTxCompose, TxCompose, TxComposeData, TxState};
use loom_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_actors_macros::{Consumer, Producer};
use loom_multicaller::SwapStepEncoder;
use loom_utils::evm::{env_for_block, evm_access_list};

use crate::estimators::tips::tips_and_value_for_swap_type;

async fn estimator_task(
    estimate_request: TxComposeData,
    encoder: SwapStepEncoder,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> Result<()> {
    debug!(
        "EVM estimation. Gas limit: {} price: {} cost: {} stuffing txs: {}",
        estimate_request.gas,
        NWETH::to_float_gwei(estimate_request.gas_fee),
        NWETH::to_float_wei(estimate_request.gas_cost()),
        estimate_request.stuffing_txs_hashes.len()
    );

    let start_time = chrono::Local::now();

    let tx_signer = estimate_request.signer.clone().ok_or(eyre!("NO_SIGNER"))?;

    let opcodes = estimate_request.opcodes.clone().ok_or(eyre!("NO_OPCODES"))?;

    let profit = estimate_request.swap.abs_profit();

    let gas_price = estimate_request.priority_gas_fee + estimate_request.gas_fee;
    let gas_cost = GasStation::calc_gas_cost(100_000, gas_price);

    // EXCHANGE SWAP
    let (tips_opcodes, call_value) = if matches!(estimate_request.swap, Swap::ExchangeSwapLine(_)) {
        debug!("Exchange swap, no tips");
        (opcodes.clone(), U256::ZERO)
    } else {
        if profit.is_zero() {
            error!("No profit for arb");
            return Err(eyre!("NO_PROFIT"));
        }

        let (tips_vec, call_value) = tips_and_value_for_swap_type(&estimate_request.swap, None, gas_cost, estimate_request.eth_balance)?;

        let mut tips_opcodes = opcodes.clone();

        for tips in tips_vec {
            tips_opcodes =
                encoder.encode_tips(tips_opcodes, tips.token_in.get_address(), tips.min_change, tips.tips, tx_signer.address())?;
        }
        (tips_opcodes, call_value)
    };
    let (to, calldata) = encoder.to_call_data(&tips_opcodes)?;

    let tx_request = TransactionRequest {
        transaction_type: Some(2),
        chain_id: Some(1),
        from: Some(tx_signer.address()),
        to: Some(TxKind::Call(to)),
        gas: Some(estimate_request.gas),
        value: Some(call_value),
        input: TransactionInput::new(calldata.clone()),
        nonce: Some(estimate_request.nonce),
        max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee),
        max_fee_per_gas: Some(estimate_request.gas_fee),
        ..TransactionRequest::default()
    };

    if let Some(db) = estimate_request.poststate {
        let evm_env = env_for_block(estimate_request.block, estimate_request.block_timestamp);
        match evm_access_list(&db, &evm_env, &tx_request) {
            Ok((gas_used, access_list)) => {
                let swap = estimate_request.swap.clone();

                if gas_used < 60_000 {
                    error!("Incorrect transaction estimation {} Gas used : {}", swap, gas_used);
                    return Err(eyre!("TRANSACTION_ESTIMATED_INCORRECTLY"));
                }

                let gas_cost = GasStation::calc_gas_cost(gas_used as u128, gas_price);

                let mut tips_vec = vec![];

                let (to, calldata, call_value) = if !matches!(estimate_request.swap, Swap::ExchangeSwapLine(_)) {
                    if gas_cost > estimate_request.swap.abs_profit_eth() {
                        error!("Profit is too small");
                        return Err(eyre!("TOO_SMALL_PROFIT"));
                    }

                    let (upd_tips_vec, call_value) =
                        tips_and_value_for_swap_type(&estimate_request.swap, None, gas_cost, estimate_request.eth_balance)?;
                    tips_vec = upd_tips_vec;

                    let call_value = if call_value.is_zero() { None } else { Some(call_value) };

                    let mut tips_opcodes = opcodes.clone();

                    for tips in tips_vec.iter() {
                        tips_opcodes = encoder.encode_tips(
                            tips_opcodes,
                            tips.token_in.get_address(),
                            tips.min_change,
                            tips.tips,
                            tx_signer.address(),
                        )?;
                    }
                    let (to, calldata) = encoder.to_call_data(&tips_opcodes)?;
                    (to, calldata, call_value)
                } else {
                    (to, calldata, None)
                };

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

                let encoded_txes: Result<Vec<TxEnvelope>, _> =
                    estimate_request.stuffing_txs.iter().map(|item| TxEnvelope::try_from(item.clone())).collect();

                let stuffing_txs_rlp: Vec<Bytes> = encoded_txes?.into_iter().map(|x| Bytes::from(x.encoded_2718())).collect();

                let mut tx_with_state: Vec<TxState> = stuffing_txs_rlp.into_iter().map(TxState::ReadyForBroadcastStuffing).collect();

                tx_with_state.push(TxState::SignatureRequired(tx_request));

                let total_tips = tips_vec.into_iter().map(|v| v.tips).sum();
                let profit_eth = estimate_request.swap.abs_profit_eth();
                let gas_cost_f64 = NWETH::to_float(gas_cost);
                let tips_f64 = NWETH::to_float(total_tips);
                let profit_eth_f64 = NWETH::to_float(profit_eth);
                let profit_f64 = match estimate_request.swap.get_first_token() {
                    Some(token_in) => token_in.to_float(estimate_request.swap.abs_profit()),
                    None => profit_eth_f64,
                };

                let sign_request = MessageTxCompose::sign(TxComposeData {
                    tx_bundle: Some(tx_with_state),
                    poststate: Some(db),
                    tips: Some(total_tips + gas_cost),
                    ..estimate_request
                });

                let result = match compose_channel_tx.send(sign_request).await {
                    Err(e) => {
                        error!("{e}");
                        Err(eyre!("COMPOSE_CHANNEL_SEND_ERROR"))
                    }
                    _ => Ok(()),
                };

                let sim_duration = chrono::Local::now() - start_time;

                //TODO add formated paths
                info!(
                    " +++ Simulation successful. Cost {} Profit {} ProfitEth {} Tips {} {}  Gas used {} Time {}",
                    gas_cost_f64, profit_f64, profit_eth_f64, tips_f64, swap, gas_used, sim_duration
                );

                result
            }
            Err(e) => {
                error!("evm_access_list error {}: {e}", estimate_request.swap);
                Err(eyre!("EVM_ACCESS_LIST_ERROR"))
            }
        }
    } else {
        error!("StateDB is None");
        Err(eyre!("STATE_DB_IS_NONE"))
    }
}

async fn estimator_worker(
    encoder: SwapStepEncoder,
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
                            tokio::task::spawn(
                                estimator_task(
                                    estimate_request,
                                    encoder.clone(),
                                    compose_channel_tx.clone(),
                                )
                            );
                        }
                    }
                    Err(e)=>{error!("{e}")}
                }
            }
        }
    }
}

#[derive(Consumer, Producer)]
pub struct EvmEstimatorActor {
    encoder: SwapStepEncoder,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl EvmEstimatorActor {
    pub fn new(encoder: SwapStepEncoder) -> Self {
        Self { encoder, compose_channel_tx: None, compose_channel_rx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { compose_channel_tx: Some(bc.compose_channel()), compose_channel_rx: Some(bc.compose_channel()), ..self }
    }
}

#[async_trait]
impl Actor for EvmEstimatorActor {
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(estimator_worker(
            self.encoder.clone(),
            self.compose_channel_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }
    fn name(&self) -> &'static str {
        "EvmEstimatorActor"
    }
}
