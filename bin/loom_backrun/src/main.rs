use alloy_provider::Provider;
use eyre::Result;
use log::{error, info};

use defi_actors::{
    ArbSwapPathEncoderActor, ArbSwapPathMergerActor, DiffPathMergerActor, SamePathMergerActor, StateChangeArbActor,
    StateHealthMonitorActor, StuffingTxMonitorActor,
};
use defi_events::MarketEvents;
use loom_actors::{Accessor, Actor, Consumer, Producer};
use loom_topology::{Topology, TopologyConfig};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("debug,tokio_tungstenite=off,tungstenite=off,alloy_rpc_client=off"),
    )
    .format_timestamp_micros()
    .init();

    let topology_config = TopologyConfig::load_from_file("config.toml".to_string())?;
    let (topology, mut worker_task_vec) = Topology::from(topology_config).await?;

    let client = topology.get_client(Some("local".to_string()).as_ref())?;
    let blockchain = topology.get_blockchain(Some("mainnet".to_string()).as_ref())?;
    let tx_signers = topology.get_signers(Some("env_signer".to_string()).as_ref())?;

    let block_nr = client.get_block_number().await?;
    info!("Block : {}", block_nr);

    info!("Creating shared state");

    info!("Starting state change arb actor");
    let mut state_change_arb_actor = StateChangeArbActor::new(client.clone(), true, true);
    match state_change_arb_actor
        .access(blockchain.mempool())
        .access(blockchain.latest_block())
        .access(blockchain.market())
        .access(blockchain.market_state())
        .access(blockchain.block_history())
        .consume(blockchain.market_events_channel())
        .consume(blockchain.mempool_events_channel())
        .produce(blockchain.compose_channel())
        .produce(blockchain.pool_health_monitor_channel())
        .start()
    {
        Err(e) => {
            error!("{}", e)
        }
        Ok(r) => {
            worker_task_vec.extend(r);
            info!("State change arb actor started successfully")
        }
    }

    let multicaller = topology.get_encoder(None).unwrap().get_multicaller();
    info!("Starting swap path encoder actor with multicaller at : {}", multicaller);

    let mut swap_path_encoder_actor = ArbSwapPathEncoderActor::new(multicaller);

    match swap_path_encoder_actor
        .access(tx_signers.clone())
        .access(blockchain.nonce_and_balance())
        .consume(blockchain.compose_channel())
        .produce(blockchain.compose_channel())
        .start()
    {
        Ok(r) => {
            worker_task_vec.extend(r);
            info!("Swap path encoder actor started successfully")
        }
        Err(e) => {
            panic!("ArbSwapPathEncoderActor {}", e)
        }
    }

    info!("Starting swap path merger actor");
    let mut swap_path_merger_actor = ArbSwapPathMergerActor::new(multicaller);

    match swap_path_merger_actor
        .access(blockchain.latest_block())
        .consume(blockchain.market_events_channel())
        .consume(blockchain.compose_channel())
        .produce(blockchain.compose_channel())
        .start()
    {
        Ok(r) => {
            worker_task_vec.extend(r);
            info!("Swap path merger actor started successfully")
        }
        Err(e) => {
            panic!("{}", e)
        }
    }

    let mut same_path_merger_actor = SamePathMergerActor::new(client.clone());

    match same_path_merger_actor
        .access(blockchain.market_state())
        .access(blockchain.latest_block())
        .consume(blockchain.market_events_channel())
        .consume(blockchain.compose_channel())
        .produce(blockchain.compose_channel())
        .start()
    {
        Ok(r) => {
            worker_task_vec.extend(r);
            info!("Same path merger actor started successfully")
        }
        Err(e) => {
            panic!("{}", e)
        }
    }

    // Merger
    let mut diff_path_merger_actor = DiffPathMergerActor::new();

    match diff_path_merger_actor
        .consume(blockchain.market_events_channel())
        .consume(blockchain.compose_channel())
        .produce(blockchain.compose_channel())
        .start()
    {
        Ok(r) => {
            worker_task_vec.extend(r);
            info!("Diff path merger actor started successfully")
        }
        Err(e) => {
            panic!("{}", e)
        }
    }

    // Monitoring pool state health, disabling pool if there are problems
    let mut state_health_monitor_actor = StateHealthMonitorActor::new(client.clone());

    match state_health_monitor_actor
        .access(blockchain.market_state())
        .consume(blockchain.compose_channel())
        .consume(blockchain.market_events_channel())
        .start()
    {
        Err(e) => {
            panic!("State health monitor actor failed : {}", e)
        }
        Ok(r) => {
            worker_task_vec.extend(r);
            info!("State health monitor actor started successfully")
        }
    }

    // Monitoring transactions we tried to attach to.
    let mut stuffing_txs_monitor_actor = StuffingTxMonitorActor::new(client.clone());
    match stuffing_txs_monitor_actor
        .access(blockchain.latest_block())
        .consume(blockchain.compose_channel())
        .consume(blockchain.market_events_channel())
        .start()
    {
        Err(e) => {
            panic!("Stuffing txs monitor actor failed : {}", e)
        }
        Ok(r) => {
            worker_task_vec.extend(r);
            info!("Stuffing txs monitor actor started successfully")
        }
    }

    // Checking workers, logging if some close
    tokio::task::spawn(async move {
        while !worker_task_vec.is_empty() {
            let (result, _index, remaining_futures) = futures::future::select_all(worker_task_vec).await;
            match result {
                Ok(work_result) => match work_result {
                    Ok(s) => {
                        info!("ActorWorker {_index} finished : {s}")
                    }
                    Err(e) => {
                        error!("ActorWorker {_index} error : {e}")
                    }
                },
                Err(e) => {
                    error!("ActorWorker join error {_index} : {e}")
                }
            }
            worker_task_vec = remaining_futures;
        }
    });

    // listening to MarketEvents in an infinite loop
    let mut s = blockchain.market_events_channel().subscribe().await;
    loop {
        let msg = s.recv().await;
        if let Ok(msg) = msg {
            match msg {
                MarketEvents::BlockTxUpdate { block_number, block_hash } => {
                    info!("New block received {} {}", block_number, block_hash);
                }
                MarketEvents::BlockStateUpdate { block_hash } => {
                    info!("New block state received {}", block_hash);
                }
                _ => {}
            }
        }
    }
}
