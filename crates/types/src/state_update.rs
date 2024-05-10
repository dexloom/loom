use std::collections::BTreeMap;

use alloy_primitives::{Address, TxHash};
use alloy_provider::ext::DebugApi;
use alloy_provider::network::Ethereum;
use alloy_provider::Provider;
use alloy_rpc_types::{BlockId, BlockNumberOrTag, TransactionRequest};
use alloy_rpc_types_trace::geth::{AccountState, GethDebugBuiltInTracerType, GethDebugTracerConfig, GethDebugTracerType, GethDebugTracingCallOptions, GethDebugTracingOptions, GethDefaultTracingOptions, GethTrace, PreStateConfig, PreStateFrame};
use alloy_rpc_types_trace::geth::GethDebugBuiltInTracerType::PreStateTracer;
use alloy_rpc_types_trace::geth::GethDebugTracerType::BuiltInTracer;
use alloy_transport::BoxTransport;
use eyre::Result;
use lazy_static::lazy_static;
use log::{info, trace};

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
            client.debug_trace_block_by_number(block_number, tracer_opts).await?
        }
        BlockId::Hash(rpc_block_hash) => {
            //client.debug_trace_block_by_number(BlockNumber::from(19776525u32), tracer_opts).await?
            client.debug_trace_block_by_hash(rpc_block_hash.block_hash, tracer_opts).await?
        }
    };


    trace!("block trace {}", trace_result_vec.len());

    let mut pre: Vec<BTreeMap<Address, AccountState>> = Vec::new();
    let mut post: Vec<BTreeMap<Address, AccountState>> = Vec::new();


    for trace_result in trace_result_vec.into_iter() {
        match trace_result.result {
            GethTrace::PreStateTracer(geth_trace_frame) => {
                match geth_trace_frame {
                    PreStateFrame::Diff(diff_frame) => {
                        pre.push(diff_frame.pre);
                        post.push(diff_frame.post);
                    }
                    PreStateFrame::Default(diff_frame) => {
                        pre.push(diff_frame.0.into_iter().collect());
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


    let tracer_call_opts = GethDebugTracingCallOptions {
        tracing_options: tracer_opts.clone(),
        state_overrides: opts.clone().and_then(|x| x.state_overrides),
        block_overrides: opts.and_then(|x| x.block_overrides),

    };


    let trace_result = client.geth_debug_trace_call(req.into(), block, tracer_call_opts).await?;
    info!("{} {} {:?}", tracer_opts.config.is_stack_enabled(), tracer_opts.config.is_storage_enabled(),trace_result);

    match trace_result {
        GethTrace::PreStateTracer(geth_trace_frame) => {
            match geth_trace_frame {
                PreStateFrame::Diff(diff_frame) => {
                    Ok((
                        diff_frame.pre,
                        diff_frame.post))
                }
                PreStateFrame::Default(diff_frame) => {
                    Ok((
                        diff_frame.0,
                        BTreeMap::new()))
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


    let trace_result = client.debug_trace_transaction(req, tracer_opts).await?;
    trace!("{:?}", trace_result);

    match trace_result {
        GethTrace::PreStateTracer(geth_trace_frame) => {
            match geth_trace_frame {
                PreStateFrame::Diff(diff_frame) => {
                    Ok((
                        diff_frame.pre.into_iter().collect(),
                        diff_frame.post.into_iter().collect()))
                }
                PreStateFrame::Default(diff_frame) => {
                    Ok((
                        diff_frame.0.into_iter().collect(),
                        BTreeMap::new()))
                }
            }
        }
        _ => {
            Err(eyre::eyre!("TRACE_RESULT_FAILED"))
        }
    }
}

#[cfg(test)]
mod test {
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::{ClientBuilder, WsConnect};
    use env_logger::Env as EnvLog;

    use super::*;

    #[tokio::test]
    async fn test_debug_block() -> Result<()> {
        std::env::set_var("RUST_LOG", "trace,tokio_tungstenite=off,tungstenite=off");
        std::env::set_var("RUST_BACKTRACE", "1");
        //let node_url = std::env::var("TEST_NODE_URL").unwrap_or("http://falcon.loop:8008/rpc".to_string());
        let node_url = std::env::var("TEST_NODE_URL").unwrap_or("ws://tokyo.loop:8008/looper".to_string());

        env_logger::init_from_env(EnvLog::default().default_filter_or("debug"));
        let node_url = url::Url::parse(node_url.as_str())?;

        //let client = ClientBuilder::default().http(node_url).boxed();
        let ws_connect = WsConnect::new(node_url);
        let client = ClientBuilder::default().ws(ws_connect).await?;

        let client = ProviderBuilder::new().on_client(client).boxed();

        let tracer_opts = GethDebugTracingOptions {
            config: GethDefaultTracingOptions::default(),
            ..GethDebugTracingOptions::default()
        }.with_tracer(BuiltInTracer(PreStateTracer)).prestate_config(PreStateConfig { diff_mode: Some(true) });

        let blocknumber = client.get_block_number().await?;
        let block = client.get_block_by_number(blocknumber.into(), false).await?.unwrap();
        //let blockhash = block.header.hash.unwrap();

        let ret = client.debug_trace_block_by_number(blocknumber.into(), tracer_opts).await?;
        //let blockhash: BlockHash = "0xd16074b40a4cb1e0b24fea1ffb5dcadb7363d38f93a9efa9eb43fc161a7e16f6".parse()?;
        //let ret = client.debug_trace_block_by_hash(blockhash, tracer_opts).await?;
        Ok(())
    }
}