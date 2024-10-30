use std::sync::Arc;

use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Encodable2718;
use alloy_network::Ethereum;
use alloy_primitives::{Bytes, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use alloy_transport::Transport;
use eyre::{eyre, Result};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info};

use defi_blockchain::Blockchain;
use defi_entities::{Swap, SwapEncoder};
use loom_utils::NWETH;

use defi_events::{MessageTxCompose, TxCompose, TxComposeData, TxState};
use flashbots::Flashbots;
use loom_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_actors_macros::{Consumer, Producer};

async fn estimator_task<T: Transport + Clone, P: Provider<T, Ethereum> + Send + Sync + Clone + 'static>(
    estimate_request: TxComposeData,
    client: Arc<Flashbots<P, T>>,
    swap_encoder: impl SwapEncoder,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> Result<()> {
    let token_in = estimate_request.swap.get_first_token().cloned().ok_or(eyre!("NO_TOKEN"))?;

    let tx_signer = estimate_request.signer.clone().ok_or(eyre!("NO_SIGNER"))?;

    let profit = estimate_request.swap.abs_profit();
    if profit.is_zero() {
        return Err(eyre!("NO_PROFIT"));
    }

    let profit_eth = token_in.calc_eth_value(profit).ok_or(eyre!("CALC_ETH_VALUE_FAILED"))?;

    let gas_price = estimate_request.priority_gas_fee + estimate_request.next_block_base_fee;
    let gas_cost = U256::from(100_000 * gas_price);

    let (to, _, call_data, _) = swap_encoder.encode(
        estimate_request.swap.clone(),
        estimate_request.tips_pct,
        Some(estimate_request.next_block_number),
        Some(gas_cost),
        Some(tx_signer.address()),
        Some(estimate_request.eth_balance),
    )?;

    let mut tx_request = TransactionRequest {
        transaction_type: Some(2),
        chain_id: Some(1),
        from: Some(tx_signer.address()),
        to: Some(TxKind::Call(to)),
        gas: Some(estimate_request.gas),
        value: Some(U256::from(1000)),
        nonce: Some(estimate_request.nonce),
        max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee as u128),
        max_fee_per_gas: Some(estimate_request.next_block_base_fee as u128),
        input: TransactionInput::new(call_data.clone()),
        ..TransactionRequest::default()
    };

    let gas_price = estimate_request.priority_gas_fee + estimate_request.next_block_base_fee;

    if U256::from(200_000 * gas_price) > profit_eth {
        error!("Profit is too small");
        return Err(eyre!("TOO_SMALL_PROFIT"));
    }

    let encoded_txes: Result<Vec<TxEnvelope>, _> =
        estimate_request.stuffing_txs.iter().map(|item| TxEnvelope::try_from(item.clone())).collect();

    let stuffing_txs_rlp: Vec<Bytes> = encoded_txes?.into_iter().map(|x| Bytes::from(x.encoded_2718())).collect();

    let mut simulation_bundle = stuffing_txs_rlp.clone();

    //let typed_tx = tx_request.clone().into();
    let (tx_hash, tx_rlp) = tx_signer.sign(tx_request.clone()).await?;
    simulation_bundle.push(tx_rlp);

    let start_time = chrono::Local::now();

    match client.simulate_txes(simulation_bundle, estimate_request.next_block_number, Some(vec![tx_hash])).await {
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
                                Some(estimate_request.next_block_number),
                                Some(gas_cost),
                                Some(tx_signer.address()),
                                Some(estimate_request.eth_balance),
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
                            nonce: Some(estimate_request.nonce),
                            access_list: Some(access_list),
                            max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee as u128),
                            max_fee_per_gas: Some(estimate_request.next_block_base_fee as u128), // TODO: Why not prio + base fee?
                            ..TransactionRequest::default()
                        };

                        let mut tx_with_state: Vec<TxState> =
                            stuffing_txs_rlp.into_iter().map(TxState::ReadyForBroadcastStuffing).collect();

                        tx_with_state.push(TxState::SignatureRequired(tx_request));

                        let total_tips = tips_vec.into_iter().map(|v| v.tips).sum();

                        let sign_request = MessageTxCompose::sign(TxComposeData {
                            gas,
                            tips: Some(total_tips + gas_cost),
                            tx_bundle: Some(tx_with_state),
                            ..estimate_request
                        });

                        match compose_channel_tx.send(sign_request).await {
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
                        //TODO add formated paths
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

async fn estimator_worker<T: Transport + Clone, P: Provider<T, Ethereum> + Send + Sync + Clone + 'static>(
    client: Arc<Flashbots<P, T>>,
    encoder: impl SwapEncoder + Send + Sync + Clone + 'static,
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
pub struct GethEstimatorActor<P, T, E> {
    client: Arc<Flashbots<P, T>>,
    encoder: E,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl<P, T, E> GethEstimatorActor<P, T, E>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    E: SwapEncoder + Send + Sync + Clone + 'static,
{
    pub fn new(client: Arc<Flashbots<P, T>>, encoder: E) -> Self {
        Self { client, encoder, compose_channel_tx: None, compose_channel_rx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { compose_channel_tx: Some(bc.compose_channel()), compose_channel_rx: Some(bc.compose_channel()), ..self }
    }
}

impl<P, T, E> Actor for GethEstimatorActor<P, T, E>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    E: SwapEncoder + Send + Sync + Clone + 'static,
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
