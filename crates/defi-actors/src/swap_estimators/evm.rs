use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Bytes, TxKind, U256};
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use eyre::{eyre, Result};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, error, info};

use defi_blockchain::Blockchain;
use defi_entities::{Swap, SwapEncoder};
use loom_utils::NWETH;

use defi_events::{MessageTxCompose, TxCompose, TxComposeData, TxState};
use loom_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_actors_macros::{Consumer, Producer};
use loom_utils::evm::{env_for_block, evm_access_list};

async fn estimator_task(
    estimate_request: TxComposeData,
    swap_encoder: impl SwapEncoder,
    compose_channel_tx: Broadcaster<MessageTxCompose>,
) -> Result<()> {
    debug!(
        gas_limit = estimate_request.gas,
        base_fee = NWETH::to_float_gwei(estimate_request.next_block_base_fee as u128),
        gas_cost = NWETH::to_float_wei(estimate_request.gas_cost()),
        stuffing_txs_len = estimate_request.stuffing_txs_hashes.len(),
        "EVM estimation",
    );

    let start_time = chrono::Local::now();

    let tx_signer = estimate_request.signer.clone().ok_or(eyre!("NO_SIGNER"))?;

    let gas_price = estimate_request.priority_gas_fee + estimate_request.next_block_base_fee;

    let (to, call_value, call_data, _) = swap_encoder.encode(
        estimate_request.swap.clone(),
        estimate_request.tips_pct,
        Some(estimate_request.next_block_number),
        None,
        Some(tx_signer.address()),
        Some(estimate_request.eth_balance),
    )?;

    let tx_request = TransactionRequest {
        transaction_type: Some(2),
        chain_id: Some(1),
        from: Some(tx_signer.address()),
        to: Some(TxKind::Call(to)),
        gas: Some(estimate_request.gas),
        value: call_value,
        input: TransactionInput::new(call_data.clone()),
        nonce: Some(estimate_request.nonce),
        max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee as u128),
        max_fee_per_gas: Some(estimate_request.next_block_base_fee as u128), // Why not prio + base fee?
        ..TransactionRequest::default()
    };

    let Some(db) = estimate_request.poststate else {
        error!("StateDB is None");
        return Err(eyre!("STATE_DB_IS_NONE"));
    };

    let evm_env = env_for_block(estimate_request.next_block_number, estimate_request.next_block_timestamp);

    let (gas_used, access_list) = match evm_access_list(&db, &evm_env, &tx_request) {
        Ok((gas_used, access_list)) => (gas_used, access_list),
        Err(e) => {
            error!(
                "evm_access_list error for block_number={}, block_timestamp={}, swap={}, err={e}",
                estimate_request.next_block_number, estimate_request.next_block_timestamp, estimate_request.swap
            );
            return Err(eyre!("EVM_ACCESS_LIST_ERROR"));
        }
    };
    let swap = estimate_request.swap.clone();

    if gas_used < 60_000 {
        error!(gas_used, %swap, "Incorrect transaction estimation");
        return Err(eyre!("TRANSACTION_ESTIMATED_INCORRECTLY"));
    }

    let gas_cost = U256::from(gas_used as u128 * gas_price as u128);

    let (to, call_value, call_data, tips_vec) = match &swap {
        Swap::ExchangeSwapLine(_) => (to, None, call_data, vec![]),
        _ => {
            match swap_encoder.encode(
                estimate_request.swap.clone(),
                estimate_request.tips_pct,
                Some(estimate_request.next_block_number),
                Some(gas_cost),
                Some(tx_signer.address()),
                Some(estimate_request.eth_balance),
            ) {
                Ok((to, call_value, call_data, tips_vec)) => (to, call_value, call_data, tips_vec),
                Err(error) => {
                    error!(%error, %swap, "swap_encoder.encode");
                    return Err(error);
                }
            }
        }
    };

    let tx_request = TransactionRequest {
        transaction_type: Some(2),
        chain_id: Some(1),
        from: Some(tx_signer.address()),
        to: Some(TxKind::Call(to)),
        gas: Some((gas_used * 1200) / 1000),
        value: call_value,
        input: TransactionInput::new(call_data),
        nonce: Some(estimate_request.nonce),
        access_list: Some(access_list),
        max_priority_fee_per_gas: Some(estimate_request.priority_gas_fee as u128),
        max_fee_per_gas: Some(estimate_request.priority_gas_fee as u128 + estimate_request.next_block_base_fee as u128),
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
        Err(error) => {
            error!(%error, "compose_channel_tx.send");
            Err(eyre!("COMPOSE_CHANNEL_SEND_ERROR"))
        }
        _ => Ok(()),
    };

    let sim_duration = chrono::Local::now() - start_time;

    info!(
        cost=gas_cost_f64,
        profit=profit_f64,
        tips=tips_f64,
        gas_used,
        %swap,
        duration=sim_duration.num_milliseconds(),
        " +++ Simulation successful",
    );

    result
}

async fn estimator_worker(
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
                            let encoder_cloned = encoder.clone();
                            tokio::task::spawn(
                                async move {
                                if let Err(e) = estimator_task(
                                    estimate_request.clone(),
                                    encoder_cloned,
                                    compose_channel_tx_cloned,
                                ).await {
                                        error!("Error in EVM estimator_task: {:?}", e);
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
pub struct EvmEstimatorActor<E> {
    encoder: E,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageTxCompose>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageTxCompose>>,
}

impl<E> EvmEstimatorActor<E>
where
    E: SwapEncoder + Send + Sync + Clone + 'static,
{
    pub fn new(encoder: E) -> Self {
        Self { encoder, compose_channel_tx: None, compose_channel_rx: None }
    }

    pub fn on_bc(self, bc: &Blockchain) -> Self {
        Self { compose_channel_tx: Some(bc.compose_channel()), compose_channel_rx: Some(bc.compose_channel()), ..self }
    }
}

impl<E> Actor for EvmEstimatorActor<E>
where
    E: SwapEncoder + Clone + Send + Sync + 'static,
{
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
