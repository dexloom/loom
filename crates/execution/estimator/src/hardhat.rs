use alloy_consensus::TxEnvelope;
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Bytes, TxKind, U256};
use alloy_provider::Provider;
use alloy_rpc_types::{TransactionInput, TransactionRequest};
use eyre::{eyre, Result};
use revm::DatabaseRef;
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info};

use loom_core_actors::{subscribe, Actor, ActorResult, Broadcaster, Consumer, Producer, WorkerResult};
use loom_core_actors_macros::{Consumer, Producer};
use loom_node_debug_provider::DebugProviderExt;
use loom_types_entities::SwapEncoder;
use loom_types_events::{MessageSwapCompose, SwapComposeData, SwapComposeMessage, TxComposeData, TxState};

async fn estimator_worker<DB: DatabaseRef + Send + Sync + Clone>(
    swap_encoder: impl SwapEncoder,
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
                                    info!("Hardhat estimation");
                                    let token_in = estimate_request.swap.get_first_token().cloned().ok_or(eyre!("NO_TOKEN"))?;

                                    let tx_signer = estimate_request.tx_compose.signer.clone().ok_or(eyre!("NO_SIGNER"))?;

                                    let gas_price = estimate_request.tx_compose.priority_gas_fee + estimate_request.tx_compose.next_block_base_fee;
                                    let gas_cost = U256::from(100_000 * gas_price);

                                    let profit = estimate_request.swap.abs_profit();
                                    if profit.is_zero() {
                                        return Err(eyre!("NO_PROFIT"));
                                    }
                                    let profit_eth = token_in.calc_eth_value(profit).ok_or(eyre!("CALC_ETH_VALUE_FAILED"))?;

                                    let (to, _call_value, call_data, _) = swap_encoder.encode(
                                        estimate_request.swap.clone(),
                                        estimate_request.tips_pct,
                                        Some(estimate_request.tx_compose.next_block_number),
                                        Some(gas_cost),
                                        Some(tx_signer.address()),
                                        Some(estimate_request.tx_compose.eth_balance),
                                    )?;

                                    let tx_request = TransactionRequest {
                                        transaction_type : Some(2),
                                        chain_id : Some(1),
                                        from: Some(tx_signer.address()),
                                        to: Some(TxKind::Call(to)),
                                        gas: Some(estimate_request.tx_compose.gas),
                                        value: Some(U256::from(1000)),
                                        input: TransactionInput::new(call_data),
                                        nonce: Some(estimate_request.tx_compose.nonce ),
                                        max_priority_fee_per_gas: Some(estimate_request.tx_compose.priority_gas_fee as u128),
                                        max_fee_per_gas: Some(estimate_request.tx_compose.next_block_base_fee as u128), // TODO: Why not prio + base fee?
                                        ..TransactionRequest::default()
                                    };

                                    let gas_price = estimate_request.tx_compose.priority_gas_fee + estimate_request.tx_compose.next_block_base_fee;

                                    if U256::from(300_000 * gas_price) > profit_eth {
                                        error!("Profit is too small");
                                        return Err(eyre!("TOO_SMALL_PROFIT"));
                                    }

                                    let enveloped_txs : Vec<TxEnvelope>= estimate_request.tx_compose.stuffing_txs.iter().map(|item| item.clone().into()).collect();
                                    let stuffing_txs_rlp : Vec<Bytes> = enveloped_txs.into_iter().map(|x| Bytes::from(x.encoded_2718()) ).collect();

                                    let mut tx_with_state: Vec<TxState> = stuffing_txs_rlp.into_iter().map(TxState::ReadyForBroadcastStuffing).collect();

                                    tx_with_state.push(TxState::SignatureRequired(tx_request));

                                    let sign_request = MessageSwapCompose::ready(
                                        SwapComposeData{
                                            tx_compose: TxComposeData{
                                            tx_bundle : Some(tx_with_state),
                                        ..estimate_request.tx_compose
                                            },
                                            ..estimate_request
                                        }
                                    );

                                    if let Err(e) = compose_channel_tx.send(sign_request){
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
pub struct HardhatEstimatorActor<P, E, DB: Send + Sync + Clone + 'static> {
    client: P,
    encoder: E,
    #[consumer]
    compose_channel_rx: Option<Broadcaster<MessageSwapCompose<DB>>>,
    #[producer]
    compose_channel_tx: Option<Broadcaster<MessageSwapCompose<DB>>>,
}

impl<P, E, DB> HardhatEstimatorActor<P, E, DB>
where
    P: Provider + DebugProviderExt + Clone + Send + Sync + 'static,
    E: SwapEncoder + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone,
{
    pub fn new(client: P, encoder: E) -> Self {
        Self { client, encoder, compose_channel_tx: None, compose_channel_rx: None }
    }
}

impl<P, E, DB> Actor for HardhatEstimatorActor<P, E, DB>
where
    P: Provider + DebugProviderExt + Clone + Send + Sync + 'static,
    E: SwapEncoder + Send + Sync + Clone + 'static,
    DB: DatabaseRef + Send + Sync + Clone,
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
        "HardhatEstimatorActor"
    }
}
