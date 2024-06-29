use std::time::Duration;

use alloy::{
    providers::{Provider, ProviderBuilder},
    rpc::client::ClientBuilder,
};
use alloy::rpc::types::{Block, Header};
use eyre::Result;
use log::{error, info};
use tokio::select;
use url::Url;

use debug_provider::HttpCachedTransport;
use defi_actors::NodeBlockPlayerActor;
use defi_events::{NodeBlockLogsUpdate, NodeBlockStateUpdate};
use loom_actors::{Accessor, Actor, Broadcaster, Consumer, Producer, SharedState};

#[tokio::main]
async fn main() -> Result<()> {
    let start_block_number = 20179184;
    let end_block_number = start_block_number + 1000;

    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug,alloy_rpc_client=off,debug_provider=info,alloy_transport_http=off,hyper_util=off"));

    let transport = HttpCachedTransport::new(Url::parse("http://falcon.loop:8008/rpc")?, Some("./.cache")).await;
    transport.set_block_number(start_block_number);

    let client = ClientBuilder::default().transport(transport.clone(), true).with_poll_interval(Duration::from_millis(50));
    let provider = ProviderBuilder::new().on_client(client);

    info!("Creating channels");
    let new_block_headers_channel: Broadcaster<Header> = Broadcaster::new(10);
    let new_block_with_tx_channel: Broadcaster<Block> = Broadcaster::new(10);
    let new_block_state_update_channel: Broadcaster<NodeBlockStateUpdate> = Broadcaster::new(10);
    let new_block_logs_channel: Broadcaster<NodeBlockLogsUpdate> = Broadcaster::new(10);

    let mut node_block_player_actor = NodeBlockPlayerActor::new(provider, start_block_number, end_block_number);

    match node_block_player_actor
        .produce(new_block_headers_channel.clone())
        .produce(new_block_with_tx_channel.clone())
        .produce(new_block_state_update_channel.clone())
        .produce(new_block_logs_channel.clone())
        .start().await {
        Ok(_) => {
            info!("Node block player actor started successfully")
        }
        Err(e) => {
            error!("Error starting node block player actor: {e}")
        }
    }

    let mut header_sub = new_block_headers_channel.subscribe().await;
    let mut block_sub = new_block_with_tx_channel.subscribe().await;
    let mut logs_sub = new_block_logs_channel.subscribe().await;
    let mut state_update_sub = new_block_state_update_channel.subscribe().await;

    loop {
        select! {
            header = header_sub.recv() => {
                match header{
                    Ok(header)=>{
                        info!("Block header received : {} {}", header.number.unwrap_or_default(), header.hash.unwrap_or_default());

                    }
                    Err(e)=>{
                        error!("Error receiving headers: {e}");
                    }
                }
            }

            logs = logs_sub.recv() => {
                match logs{
                    Ok(logs_update)=>{
                        info!("Block logs received : {} log records : {}", logs_update.block_hash, logs_update.logs.len());

                    }
                    Err(e)=>{
                        error!("Error receiving logs: {e}");
                    }
                }
            }

            block = block_sub.recv() => {
                match block {
                    Ok(block)=>{
                        info!("Block with tx received : {} txs : {}", block.header.hash.unwrap_or_default(), block.transactions.len());

                    }
                    Err(e)=>{
                        error!("Error receiving blocks: {e}");
                    }
                }
            }
            state_udpate = state_update_sub.recv() => {
                match state_udpate {
                    Ok(state_update)=>{
                        info!("Block state update received : {} update records : {}", state_update.block_hash, state_update.state_update.len() );
                    }
                    Err(e)=>{
                        error!("Error receiving blocks: {e}");
                    }
                }
            }

        }
    }
    Ok(())
}
