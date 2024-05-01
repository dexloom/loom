use std::collections::BTreeMap;

use alloy_primitives::{Address, TxHash};
use alloy_provider::{Network, Provider};
use alloy_provider::ext::DebugApi;
use alloy_provider::network::Ethereum;
use alloy_rpc_types::{BlockId, BlockNumberOrTag, TransactionRequest};
use alloy_rpc_types_trace::geth::{AccountState, GethDebugBuiltInTracerType, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions, GethDebugTracingOptions, GethDefaultTracingOptions, GethTrace, PreStateConfig, PreStateFrame};
use alloy_rpc_types_trace::geth::GethDebugBuiltInTracerType::PreStateTracer;
use alloy_rpc_types_trace::geth::GethDebugTracerType::BuiltInTracer;
use alloy_transport::BoxTransport;
use eyre::{eyre, Result};
use lazy_static::lazy_static;
use log::{error, trace};

use debug_provider::DebugProviderExt;

pub type GethStateUpdate = BTreeMap<Address, AccountState>;


pub type GethStateUpdateVec = Vec<BTreeMap<Address, AccountState>>;


lazy_static! {
pub static ref TRACING_OPTS: GethDebugTracingOptions = GethDebugTracingOptions {
    tracer: Some(GethDebugTracerType::BuiltInTracer(
        GethDebugBuiltInTracerType::PreStateTracer,
    )),
    tracer_config: GethDebugTracerConfig::default(),
    config: GethDefaultTracingOptions::default().disable_storage().disable_stack().disable_memory().disable_return_data(),
    timeout: None,
};

pub static ref TRACING_CALL_OPTS: GethDebugTracingCallOptions = GethDebugTracingCallOptions {
    tracing_options: TRACING_OPTS.clone(),
    state_overrides: None,
    block_overrides: None,

};

}


pub async fn debug_trace_block<P: Provider>(client: P, block_id: BlockId, diff_mode: bool) -> eyre::Result<(GethStateUpdateVec, GethStateUpdateVec)> {
    let tracer_opts = GethDebugTracingOptions {
        config: GethDefaultTracingOptions::default(),
        ..GethDebugTracingOptions::default()
    }.with_tracer(BuiltInTracer(PreStateTracer)).prestate_config(PreStateConfig { diff_mode: Some(diff_mode) });


    let trace_result_vec = match block_id {
        BlockId::Number(block_number) => {
            client.debug_trace_block_by_number(block_number.as_number().unwrap(), tracer_opts).await?
        }
        BlockId::Hash(rpc_block_hash) => {
            client.debug_trace_block_by_hash(rpc_block_hash.block_hash, tracer_opts).await?
        }
    };


    trace!("block trace {}", trace_result_vec.len());

    let mut pre: Vec<BTreeMap<Address, AccountState>> = Vec::new();
    let mut post: Vec<BTreeMap<Address, AccountState>> = Vec::new();


    for trace_result in trace_result_vec.into_iter() {
        match trace_result {
            GethTrace::PreStateTracer(geth_trace_frame) => {
                match geth_trace_frame {
                    PreStateFrame::Diff(diff_frame) => {
                        pre.push(diff_frame.pre.into_iter().map(|(k, v)| (k, v)).collect());
                        post.push(diff_frame.post.into_iter().map(|(k, v)| (k, v)).collect());
                    }
                    PreStateFrame::Default(diff_frame) => {
                        pre.push(diff_frame.0.into_iter().map(|(k, v)| (k, v)).collect());
                    }
                }
            }
            _ => {
                return Err(eyre::eyre!("TRACE_RESULT_FAILED"));
            }
        }
    };
    Ok((pre, post))
}


async fn debug_trace_call<C: DebugProviderExt, T: Into<TransactionRequest> + Send + Sync>(client: C, req: T, block: BlockNumberOrTag, opts: Option<GethDebugTracingCallOptions>, diff_mode: bool) -> Result<(GethStateUpdate, GethStateUpdate)> {
    let tracer_opts = GethDebugTracingOptions {
        config: GethDefaultTracingOptions::default(),
        ..GethDebugTracingOptions::default()
    }.with_tracer(BuiltInTracer(PreStateTracer)).prestate_config(PreStateConfig { diff_mode: Some(diff_mode) });
// TODO : Fix parameters

    let tracer_call_opts = GethDebugTracingCallOptions {
        tracing_options: tracer_opts,
        state_overrides: opts.clone().map_or(None, |x| x.state_overrides),
        block_overrides: opts.map_or(None, |x| x.block_overrides),

    };


    let trace_result = client.geth_debug_trace_call(req.into(), block, tracer_call_opts).await?;
    trace!("{:?}", trace_result);

    match trace_result {
        GethTrace::PreStateTracer(geth_trace_frame) => {
            match geth_trace_frame {
                PreStateFrame::Diff(diff_frame) => {
                    Ok((
                        diff_frame.pre.into_iter().map(|(k, v)| (k, v)).collect(),
                        diff_frame.post.into_iter().map(|(k, v)| (k, v)).collect()))
                }
                PreStateFrame::Default(diff_frame) => {
                    Ok((
                        diff_frame.0.into_iter().map(|(k, v)| (k, v)).collect(),
                        BTreeMap::new()))
                }
                _ => {
                    Err(eyre::eyre!("PRESTATE_TRACE_FAILED"))
                }
            }
        }
        _ => {
            Err(eyre::eyre!("TRACE_RESULT_FAILED"))
        }
    }
}


pub async fn debug_trace_call_pre_state<C: DebugProviderExt, T: Into<TransactionRequest> + Send + Sync>(client: C, req: T, block: BlockNumberOrTag, opts: Option<GethDebugTracingCallOptions>) -> eyre::Result<GethStateUpdate> {
    Ok(debug_trace_call(client, req, block, opts, false).await?.0)
}


pub async fn debug_trace_call_post_state<C: DebugProviderExt, T: Into<TransactionRequest> + Send + Sync>(client: C, req: T, block: BlockNumberOrTag, opts: Option<GethDebugTracingCallOptions>) -> eyre::Result<GethStateUpdate> {
    Ok(debug_trace_call(client, req, block, opts, true).await?.1)
}

pub async fn debug_trace_call_diff<C: DebugProviderExt, T: Into<TransactionRequest> + Send + Sync>(client: C, req: T, block: BlockNumberOrTag, call_opts: Option<GethDebugTracingCallOptions>) -> eyre::Result<(GethStateUpdate, GethStateUpdate)> {
    debug_trace_call(client, req, block, call_opts, true).await
}


pub async fn debug_trace_transaction<P: Provider + DebugApi<Ethereum, BoxTransport>>(client: P, req: TxHash, diff_mode: bool) -> Result<(GethStateUpdate, GethStateUpdate)> {
    let tracer_opts = GethDebugTracingOptions {
        config: GethDefaultTracingOptions::default(),
        ..GethDebugTracingOptions::default()
    }.with_tracer(BuiltInTracer(PreStateTracer)).prestate_config(PreStateConfig { diff_mode: Some(diff_mode) });
// TODO : Fix parameters


    let trace_result = client.debug_trace_transaction(req.into(), tracer_opts).await?;
    trace!("{:?}", trace_result);

    match trace_result {
        GethTrace::PreStateTracer(geth_trace_frame) => {
            match geth_trace_frame {
                PreStateFrame::Diff(diff_frame) => {
                    Ok((
                        diff_frame.pre.into_iter().map(|(k, v)| (k, v)).collect(),
                        diff_frame.post.into_iter().map(|(k, v)| (k, v)).collect()))
                }
                PreStateFrame::Default(diff_frame) => {
                    Ok((
                        diff_frame.0.into_iter().map(|(k, v)| (k, v)).collect(),
                        BTreeMap::new()))
                }
                _ => {
                    Err(eyre::eyre!("PRESTATE_TRACE_FAILED"))
                }
            }
        }
        _ => {
            Err(eyre::eyre!("TRACE_RESULT_FAILED"))
        }
    }
}
