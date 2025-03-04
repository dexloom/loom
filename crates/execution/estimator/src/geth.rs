use revm::DatabaseRef;
use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Encodable2718;
use alloy_network::Ethereum;
use alloy_primitives::{Bytes, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use eyre::{eyre, Result};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info};

use loom_core_blockchain::Strategy;
use loom_evm_utils::NWETH;
use loom_types_entities::{Swap, SwapEncoder};

use loom_broadcast_flashbots::Flashbots;
use loom_core_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Consumer, Producer};
use loom_types_blockchain::LoomTx;
use loom_types_events::{MessageSwapCompose, SwapComposeData, SwapComposeMessage, TxComposeData, TxState};

async fn estimator_task<P: Provider<Ethereum> + Send + Sync + Clone + 'static, DB: DatabaseRef + Send + Sync + Clone>(
    estimate_request: SwapComposeData<DB>,
    client: Arc<Flashbots<P>>,
    swap_encoder: impl SwapEncoder,
    compose_channel_tx: Broadcaster<MessageSwapCompose<DB>>,
) -> Result<()> {
    let token_in = estimate_request.swap.get_first_token().cloned().ok_or(eyre!("NO_TOKEN"))?;

    let tx_signer = estimate_request.tx_compose.signer.clone().ok_or(eyre!("NO_SIGNER"))?;

    let profit = estimate_request.swap.abs_profit();
    if profit.is_zero() {
        return Err(eyre!("NO_PROFIT"));
    }

    let profit_eth = token_in.calc_eth_value(profit).ok_or(eyre!("CALC_ETH_VALUE_FAILED"))?;

    let gas_price = estimate_request.tx_compose.priority_gas_fee + estimate_request.tx_compose.next_block_base_fee;
    let gas_cost = U256::from(100_000 * gas_price);

    let (to, _, call_data, _) = swap_encoder.encode(
        estimate_request.swap.clone(),
        estimate_request.tips_pct,
        Some(estimate_request.tx_compose.next_block_number),
        Some(gas_cost),
        Some(tx_signer.address()),
        Some(estimate_request.tx_compose.eth_balance),
    )?;

    let mut tx_request = TransactionRequest {
        transaction_type: Some(2),
        chain_id: Some(1),
        from: Some(tx_signer.address()),
        to: Some(TxKind::Call(to)),
        gas: Some(estimate_request.tx_compose.gas),
        value: Some(U256::from(1000)),
        nonce: Some(estimate_request.tx_compose.nonce),
        max_priority_fee_per_gas: Some(estimate_request.tx_compose.priority_gas_fee as u128),
        max_fee_per_gas: Some(estimate_request.tx_compose.next_block_base_fee as u128),
        input: TransactionInput::new(call_data.clone()),
        ..TransactionRequest::default()
    };

    let gas_price = estimate_request.tx_compose.priority_gas_fee + estimate_request.tx_compose.next_block_base_fee;

    if U256::from(200_000 * gas_price) > profit_eth {
        error!("Profit is too small");
        return Err(eyre!("TOO_SMALL_PROFIT"));
    }

    let encoded_txes: Vec<TxEnvelope> =
        estimate_request.tx_compose.stuffing_txs.iter().map(|item| TxEnvelope::from(item.clone())).collect();

    let stuffing_txs_rlp: Vec<Bytes> = encoded_txes.into_iter().map(|x| Bytes::from(x.encoded_2718())).collect();

    let mut simulation_bundle = stuffing_txs_rlp.clone();

    //let typed_tx = tx_request.clone().into();
    let tx = tx_signer.sign(tx_request.clone()).await?;
    let tx_hash = LoomTx::tx_hash(&tx);
    let tx_rlp = tx.encode();

    simulation_bundle.push(Bytes::from(tx_rlp));

    let start_time = chrono::Local::now();

    match client.simulate_txes(simulation_bundle, estimate_request.tx_compose.next_block_number, Some(vec![tx_hash])).await {
        Ok(sim_result) => {
            let sim_duration = chrono::Local::now() - start_time;
            debug!(
                "Simulation result received Gas used : {} CB : {}  {} {}",
                sim_result.gas_used, sim_result.coinbase_tip, sim_result.coinbase_diff, sim_duration
            );
            debug!("Simulation swap step");
            for tx_sim_result in sim_result.transactions.iter() {
                let prefix = if tx_sim_result.revert.is_none() && tx_sim_result.error.is_none() { "++" } else { "--" };
                info!("{} {}", prefix, tx_sim_result);
            }

            if let Some(tx_sim_result) = sim_result.find_tx(tx_hash) {
                if let Some(error) = &tx_sim_result.error {
                    error!(" --- Simulation error : {} {}", error, sim_duration);
                    return Err(eyre!("TX_SIMULATION_ERROR"));
                }
                if let Some(revert) = &tx_sim_result.revert {
                    error!(" --- Simulation revert : {} {}", revert, sim_duration);
                    return Err(eyre!("TX_SIMULATION_REVERT"));
                }

                let gas = tx_sim_result.gas_used.to();

                if let Some(access_list) = tx_sim_result.access_list.clone() {
                    let swap = estimate_request.swap.clone();

                    tx_request.access_list = Some(access_list.clone());
                    let gas_cost = U256::from(gas * gas_price);
                    if gas_cost < profit_eth {
                        let (to, call_value, call_data, tips_vec) = match estimate_request.swap {
                            Swap::ExchangeSwapLine(_) => (to, None, call_data, vec![]),
                            _ => swap_encoder.encode(
                                estimate_request.swap.clone(),
                                estimate_request.tips_pct,
                                Some(estimate_request.tx_compose.next_block_number),
                                Some(gas_cost),
                                Some(tx_signer.address()),
                                Some(estimate_request.tx_compose.eth_balance),
                            )?,
                        };

                        let tx_request = TransactionRequest {
                            transaction_type: Some(2),
                            chain_id: Some(1),
                            from: Some(tx_signer.address()),
                            to: Some(TxKind::Call(to)),
                            gas: Some((gas * 1500) / 1000),
                            value: call_value,
                            input: TransactionInput::new(call_data),
                            nonce: Some(estimate_request.tx_compose.nonce),
                            access_list: Some(access_list),
                            max_priority_fee_per_gas: Some(estimate_request.tx_compose.priority_gas_fee as u128),
                            max_fee_per_gas: Some(estimate_request.tx_compose.next_block_base_fee as u128), // TODO: Why not prio + base fee?
                            ..TransactionRequest::default()
                        };

                        let mut tx_with_state: Vec<TxState> =
                            stuffing_txs_rlp.into_iter().map(TxState::ReadyForBroadcastStuffing).collect();

                        tx_with_state.push(TxState::SignatureRequired(tx_request));

                        let total_tips = tips_vec.into_iter().map(|v| v.tips).sum();

                        let sign_request = MessageSwapCompose::ready(SwapComposeData {
                            tx_compose: TxComposeData { gas, ..estimate_request.tx_compose },
                            tips: Some(total_tips + gas_cost),
                            ..estimate_request
                        });

                        match compose_channel_tx.send(sign_request) {
                            Ok(_) => {
                                info!("Simulated bundle broadcast to flashbots")
                            }
                            Err(e) => {
                                error!("{}", e)
                            }
                        }

                        let gas_cost_f64 = NWETH::to_float(gas_cost);
                        let tips_f64 = NWETH::to_float(total_tips);
                        let profit_eth_f64 = NWETH::to_float(profit_eth);
                        let profit_f64 = token_in.to_float(profit);
                        info!(
                            " +++ Simulation successful. {:#32x} Cost {} Profit {} ProfitEth {} Tips {} {} {} {}",
                            tx_hash, gas_cost_f64, profit_f64, profit_eth_f64, tips_f64, tx_sim_result, swap, sim_duration
                        )
                    } else {
                        error!(" --- Simulation error : profit does not cover gas cost {} {} {}", gas_cost, profit, sim_duration);
                        return Err(eyre!("BAD_PROFIT"));
                    }
                } else {
                    error!(" --- Simulation error : Access list not found in simulated transaction");
                    return Err(eyre!("ACL_NOT_FOUND_IN_SIMULATION"));
                }
            } else {
                error!("Simulation error : Transaction not found in simulated bundle");
                return Err(eyre!("TX_NOT_FOUND_IN_SIMULATION"));
            }
        }
        Err(e) => {
            error!("Simulation error {}", e);
            return Err(eyre!("SIMULATION_ERROR"));
        }
    }

    Ok(())
}

async fn estimator_worker<P: Provider<Ethereum> + Send + Sync + Clone + 'static, DB: DatabaseRef + Send + Sync + Clone>(
    client: Arc<Flashbots<P>>,
    encoder: impl SwapEncoder + Send + Sync + Clone + 'static,
    compose_channel_rx: Broadcaster<MessageSwapCompose<DB>>,
    compose_channel_tx: Broadcaster<MessageSwapCompose<DB>>,
) -> WorkerResult {
    subscribe!(compose_channel_rx);

    loop {
        tokio::select! {
            msg = compose_channel_rx.recv() => {
                let compose_request_msg : Result<MessageSwapCompose<DB>, RecvError> = msg;
                match compose_request_msg {
                    Ok(compose_request) =>{
                        if let SwapComposeMessage::Estimate(estimate_request) = compose_request.inner {
                            let compose_channel_tx_cloned = compose_channel_tx.clone();
                            let client_cloned = client.clone();
                            let encoder_cloned = encoder.clone();
                            tokio::task::spawn(async move {
                                if let Err(e) = estimator_task(
                                    estimate_request.clone(),
                                    client_cloned,
                                    encoder_cloned,
                                    compose_channel_tx_cloned,
                                ).await {
                                        error!("Error in Geth estimator_task: {:?}", e);
                                    }
                                }
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
pub struct GethEstimatorActor<P, E, DB: Clone + Send + Sync + 'static> {
    client: Arc<Flashbots<P>>,
    encoder: E,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageSwapCompose<DB>>>,
}

impl<P, E, DB> GethEstimatorActor<P, E, DB>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
    E: SwapEncoder + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone,
{
    pub fn new(client: Arc<Flashbots<P>>, encoder: E) -> Self {
        Self { client, encoder, compose_channel_tx: None, compose_channel_rx: None }
    }

    pub fn on_bc(self, strategy: &Strategy<DB>) -> Self {
        Self {
            compose_channel_tx: Some(strategy.swap_compose_channel()),
            compose_channel_rx: Some(strategy.swap_compose_channel()),
            ..self
        }
    }
}

impl<P, E, DB> Actor for GethEstimatorActor<P, E, DB>
where
    P: Provider<Ethereum> + Send + Sync + Clone + 'static,
    E: SwapEncoder + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone,
{
    fn start(&self) -> ActorResult {
        let task = tokio::task::spawn(estimator_worker(
            self.client.clone(),
            self.encoder.clone(),
            self.compose_channel_rx.clone().unwrap(),
            self.compose_channel_tx.clone().unwrap(),
        ));
        Ok(vec![task])
    }

    fn name(&self) -> &'static str {
        "GethEstimatorActor"
    }
}
