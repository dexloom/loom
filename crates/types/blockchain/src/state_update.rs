use alloy_primitives::{Address, TxHash};
use alloy_provider::ext::DebugApi;
use alloy_provider::{Network, Provider};
use alloy_rpc_types::{BlockId, TransactionRequest};
use alloy_rpc_types_trace::common::TraceResult;
use alloy_rpc_types_trace::geth::GethDebugBuiltInTracerType::PreStateTracer;
use alloy_rpc_types_trace::geth::GethDebugTracerType::BuiltInTracer;
use alloy_rpc_types_trace::geth::{
    AccountState, GethDebugBuiltInTracerType, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions,
    GethDebugTracingOptions, GethDefaultTracingOptions, GethTrace, PreStateConfig, PreStateFrame,
};
use eyre::Result;
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use tracing::{debug, trace};

use loom_node_debug_provider::DebugProviderExt;

pub type GethStateUpdate = BTreeMap<Address, AccountState>;

pub type GethStateUpdateVec = Vec<BTreeMap<Address, AccountState>>;

lazy_static! {
    pub static ref TRACING_OPTS: GethDebugTracingOptions = GethDebugTracingOptions {
        tracer: Some(GethDebugTracerType::BuiltInTracer(GethDebugBuiltInTracerType::PreStateTracer,)),
        tracer_config: GethDebugTracerConfig::default(),
        config: GethDefaultTracingOptions::default().disable_storage().disable_stack().disable_memory().disable_return_data(),
        timeout: None,
    };
    pub static ref TRACING_CALL_OPTS: GethDebugTracingCallOptions =
        GethDebugTracingCallOptions { tracing_options: TRACING_OPTS.clone(), state_overrides: None, block_overrides: None };
}

pub fn get_touched_addresses(state_update: &GethStateUpdate) -> Vec<Address> {
    let mut ret: Vec<Address> = Vec::new();

    for (address, state) in state_update.iter() {
        if !state.storage.is_empty() {
            ret.push(*address)
        }
    }

    ret
}

pub fn debug_log_geth_state_update(state_update: &GethStateUpdate) {
    for (address, state) in state_update {
        debug!("{} nonce {:?} balance {:?} is_code {}", address, state.nonce, state.balance, state.code.is_some())
    }
}

pub async fn debug_trace_block<N: Network, P: Provider<N> + DebugProviderExt<N>>(
    client: P,
    block_id: BlockId,
    diff_mode: bool,
) -> eyre::Result<(GethStateUpdateVec, GethStateUpdateVec)> {
    let tracer_opts = GethDebugTracingOptions { config: GethDefaultTracingOptions::default(), ..GethDebugTracingOptions::default() }
        .with_tracer(BuiltInTracer(PreStateTracer))
        .with_prestate_config(PreStateConfig { diff_mode: Some(diff_mode), disable_code: Some(false), disable_storage: Some(false) });

    let trace_result_vec = match block_id {
        BlockId::Number(block_number) => client.geth_debug_trace_block_by_number(block_number, tracer_opts).await?,
        BlockId::Hash(rpc_block_hash) => {
            //client.debug_trace_block_by_number(BlockNumber::from(19776525u32), tracer_opts).await?
            client.geth_debug_trace_block_by_hash(rpc_block_hash.block_hash, tracer_opts).await?
        }
    };

    trace!("block trace {}", trace_result_vec.len());

    let mut pre: GethStateUpdateVec = Default::default();
    let mut post: GethStateUpdateVec = Default::default();

    for trace_result in trace_result_vec.into_iter() {
        if let TraceResult::Success { result, .. } = trace_result {
            match result {
                GethTrace::PreStateTracer(geth_trace_frame) => match geth_trace_frame {
                    PreStateFrame::Diff(diff_frame) => {
                        pre.push(diff_frame.pre);
                        post.push(diff_frame.post);
                    }
                    PreStateFrame::Default(diff_frame) => {
                        pre.push(diff_frame.0.into_iter().collect());
                    }
                },
                _ => {
                    return Err(eyre::eyre!("TRACE_RESULT_FAILED"));
                }
            }
        }
    }
    Ok((pre, post))
}

async fn debug_trace_call<N: Network, C: DebugProviderExt<N>, TR: Into<TransactionRequest> + Send + Sync>(
    client: C,
    req: TR,
    block: BlockId,
    opts: Option<GethDebugTracingCallOptions>,
    diff_mode: bool,
) -> Result<(GethStateUpdate, GethStateUpdate)> {
    let tracer_opts = GethDebugTracingOptions { config: GethDefaultTracingOptions::default(), ..GethDebugTracingOptions::default() }
        .with_tracer(BuiltInTracer(PreStateTracer))
        .with_prestate_config(PreStateConfig { diff_mode: Some(diff_mode), disable_code: Some(false), disable_storage: Some(false) });

    let tracer_call_opts = GethDebugTracingCallOptions {
        tracing_options: tracer_opts.clone(),
        state_overrides: opts.clone().and_then(|x| x.state_overrides),
        block_overrides: opts.and_then(|x| x.block_overrides),
    };

    let trace_result = client.geth_debug_trace_call(req.into(), block, tracer_call_opts.clone()).await?;
    trace!(
        "{} {} {:?} {:?}",
        tracer_opts.config.is_stack_enabled(),
        tracer_opts.config.is_storage_enabled(),
        tracer_call_opts.clone(),
        trace_result
    );

    match trace_result {
        GethTrace::PreStateTracer(geth_trace_frame) => match geth_trace_frame {
            PreStateFrame::Diff(diff_frame) => Ok((diff_frame.pre, diff_frame.post)),
            PreStateFrame::Default(diff_frame) => Ok((diff_frame.0, Default::default())),
        },
        _ => Err(eyre::eyre!("TRACE_RESULT_FAILED")),
    }
}

pub async fn debug_trace_call_pre_state<N: Network, C: DebugProviderExt<N>, TR: Into<TransactionRequest> + Send + Sync>(
    client: C,
    req: TR,
    block: BlockId,
    opts: Option<GethDebugTracingCallOptions>,
) -> eyre::Result<GethStateUpdate> {
    Ok(debug_trace_call(client, req, block, opts, false).await?.0)
}

pub async fn debug_trace_call_post_state<N: Network, C: DebugProviderExt<N>, TR: Into<TransactionRequest> + Send + Sync>(
    client: C,
    req: TR,
    block: BlockId,
    opts: Option<GethDebugTracingCallOptions>,
) -> eyre::Result<GethStateUpdate> {
    Ok(debug_trace_call(client, req, block, opts, true).await?.1)
}

pub async fn debug_trace_call_diff<N: Network, C: DebugProviderExt<N>, TR: Into<TransactionRequest> + Send + Sync>(
    client: C,
    req: TR,
    block: BlockId,
    call_opts: Option<GethDebugTracingCallOptions>,
) -> eyre::Result<(GethStateUpdate, GethStateUpdate)> {
    debug_trace_call(client, req, block, call_opts, true).await
}

pub async fn debug_trace_transaction<N: Network, P: Provider<N> + DebugApi<N>>(
    client: P,
    req: TxHash,
    diff_mode: bool,
) -> Result<(GethStateUpdate, GethStateUpdate)> {
    let tracer_opts = GethDebugTracingOptions { config: GethDefaultTracingOptions::default(), ..GethDebugTracingOptions::default() }
        .with_tracer(BuiltInTracer(PreStateTracer))
        .with_prestate_config(PreStateConfig { diff_mode: Some(diff_mode), disable_code: Some(false), disable_storage: Some(false) });

    let trace_result = client.debug_trace_transaction(req, tracer_opts).await?;
    trace!("{:?}", trace_result);

    match trace_result {
        GethTrace::PreStateTracer(geth_trace_frame) => match geth_trace_frame {
            PreStateFrame::Diff(diff_frame) => Ok((diff_frame.pre.into_iter().collect(), diff_frame.post.into_iter().collect())),
            PreStateFrame::Default(diff_frame) => Ok((diff_frame.0.into_iter().collect(), Default::default())),
        },
        _ => Err(eyre::eyre!("TRACE_RESULT_FAILED")),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy_primitives::map::B256HashMap;
    use alloy_primitives::{B256, U256};
    use alloy_provider::network::primitives::BlockTransactionsKind;
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::{ClientBuilder, WsConnect};
    use alloy_rpc_types::state::{AccountOverride, StateOverride};
    use env_logger::Env as EnvLog;
    use tracing::{debug, error};

    #[tokio::test]
    async fn test_debug_block() -> Result<()> {
        let node_url = std::env::var("MAINNET_WS")?;

        let _ = env_logger::try_init_from_env(EnvLog::default().default_filter_or("info,tokio_tungstenite=off,tungstenite=off"));
        let node_url = url::Url::parse(node_url.as_str())?;

        let ws_connect = WsConnect::new(node_url);
        let client = ClientBuilder::default().ws(ws_connect).await?;

        let client = ProviderBuilder::new().disable_recommended_fillers().on_client(client);

        let blocknumber = client.get_block_number().await?;
        let _block = client.get_block_by_number(blocknumber.into(), BlockTransactionsKind::Hashes).await?.unwrap();

        let _ret = debug_trace_block(client, BlockId::Number(blocknumber.into()), true).await?;

        Ok(())
    }

    #[test]
    fn test_encode_override() {
        let mut state_override: StateOverride = StateOverride::default();
        let address = Address::default();
        let mut account_override: AccountOverride = AccountOverride::default();
        let mut state_update_hashmap: B256HashMap<B256> = B256HashMap::default();
        state_update_hashmap.insert(B256::from(U256::from(1)), B256::from(U256::from(3)));
        account_override.state_diff = Some(state_update_hashmap);

        state_override.insert(address, account_override);

        match serde_json::to_string_pretty(&state_override) {
            Ok(data) => {
                debug!("{}", data);
            }
            Err(e) => {
                error!("{}", e);
                panic!("DESERIALIZATION_ERROR");
            }
        }
    }
}
