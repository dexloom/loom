use crate::cli::Cli;
use alloy::network::primitives::BlockTransactionsKind;
use alloy::primitives::{BlockHash, BlockNumber};
use alloy::{
    eips::BlockNumberOrTag,
    primitives::TxHash,
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::BlockTransactions,
};
use chrono::{DateTime, Duration, Local, TimeDelta};
use clap::Parser;
use eyre::{eyre, Result};
use futures::future::join_all;
use loom_core_blockchain::{Blockchain, BlockchainState, Strategy};
use loom_core_blockchain_actors::BlockchainActors;
use loom_evm_db::LoomDB;
use loom_execution_multicaller::MulticallerSwapEncoder;
use loom_node_actor_config::NodeBlockActorConfig;
use loom_types_events::MempoolEvents;
use std::fmt::Formatter;
use std::{collections::HashMap, fmt::Display, sync::Arc};
use tokio::{select, sync::RwLock, task::JoinHandle};

mod cli;

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct StatEntry {
    first: Vec<usize>,
    total_delay: Vec<Duration>,
    avg_delay_ms: Vec<i64>,
}

impl Display for StatEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "first {:?} avg delay {:?} μs", self.first, self.avg_delay_ms)
    }
}

#[derive(Clone, Debug, Default)]
pub struct TimeMap {
    time_map: HashMap<usize, DateTime<Local>>,
}

impl TimeMap {
    pub fn add_time(&mut self, id: usize, time: DateTime<Local>) {
        self.time_map.entry(id).or_insert(time);
    }
    pub fn add_now(&mut self, id: usize) -> DateTime<Local> {
        *self.time_map.entry(id).or_insert(Local::now())
    }

    pub fn get_time(&self, id: usize) -> Option<&DateTime<Local>> {
        self.time_map.get(&id)
    }

    pub fn to_relative(&self, pings: &[TimeDelta]) -> TimeMap {
        let rel_time: HashMap<usize, DateTime<Local>> =
            self.time_map.iter().map(|(k, v)| (*k, *v - pings.get(*k).cloned().unwrap())).collect();
        TimeMap { time_map: rel_time }
    }

    pub fn get_first_time(&self) -> DateTime<Local> {
        self.time_map.values().min().cloned().unwrap_or_default()
    }

    pub fn get_time_delta(&self, id: usize) -> Option<TimeDelta> {
        self.time_map.get(&id).map(|x| *x - self.get_first_time())
    }
}

fn analyze_time_maps(time_map_vec: Vec<&TimeMap>, ping: Option<&[TimeDelta]>) -> StatEntry {
    let nodes_count = time_map_vec.first();

    if nodes_count.is_none() {
        return Default::default();
    }

    let nodes_count = nodes_count.unwrap().time_map.len();
    if nodes_count == 0 {
        return Default::default();
    }

    let mut delays: Vec<Duration> = vec![Duration::default(); nodes_count];
    let mut received_first: Vec<usize> = vec![0; nodes_count];

    for time_map in time_map_vec.iter() {
        for node_id in 0..nodes_count {
            match ping {
                Some(ping) => {
                    if let Some(t) = time_map.to_relative(ping).get_time_delta(node_id) {
                        delays[node_id] += t;
                        if t.is_zero() {
                            received_first[node_id] += 1;
                        }
                    }
                }
                None => {
                    if let Some(t) = time_map.get_time_delta(node_id) {
                        delays[node_id] += t;
                        if t.is_zero() {
                            received_first[node_id] += 1;
                        }
                    }
                }
            }
        }
    }

    let total_entries: usize = received_first.iter().sum();

    let delays_avg: Vec<i64> = delays
        .iter()
        .enumerate()
        .map(|(i, x)| {
            if total_entries - received_first[i] == 0 {
                0
            } else {
                x.num_microseconds().unwrap_or_default() / ((total_entries - received_first[i]) as i64)
            }
        })
        .collect();

    StatEntry { first: received_first, total_delay: delays, avg_delay_ms: delays_avg }
}

#[derive(Clone, Debug, Default)]
pub struct StatCollector {
    ping: Vec<TimeDelta>,
    blocks: HashMap<BlockHash, BlockNumber>,
    block_headers: HashMap<BlockNumber, TimeMap>,
    block_with_tx: HashMap<BlockNumber, TimeMap>,
    block_logs: HashMap<BlockNumber, TimeMap>,
    block_state: HashMap<BlockNumber, TimeMap>,
    txs: HashMap<TxHash, TimeMap>,
}

impl Display for StatCollector {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "headers abs {}", analyze_time_maps(self.block_headers.values().collect(), None))?;
        writeln!(f, "headers rel {}", analyze_time_maps(self.block_headers.values().collect(), Some(&self.ping)))?;
        writeln!(f, "blocks abs {}", analyze_time_maps(self.block_with_tx.values().collect(), None))?;
        writeln!(f, "blocks rel {}", analyze_time_maps(self.block_with_tx.values().collect(), Some(&self.ping)))?;
        writeln!(f, "logs abs {}", analyze_time_maps(self.block_logs.values().collect(), None))?;
        writeln!(f, "logs rel {}", analyze_time_maps(self.block_logs.values().collect(), Some(&self.ping)))?;
        writeln!(f, "state abs {}", analyze_time_maps(self.block_state.values().collect(), None))?;
        writeln!(f, "state rel {}", analyze_time_maps(self.block_state.values().collect(), Some(&self.ping)))?;
        writeln!(f, "-----")
    }
}

#[derive(Clone, Debug, Default)]
pub struct TxStatCollector {
    pub(crate) total_received_tx: usize,
    pub(crate) total_txs: usize,
    pub(crate) txs_received: Vec<usize>,
    pub(crate) txs_received_first: Vec<usize>,
    pub(crate) txs_received_first_relative: Vec<usize>,
    pub(crate) txs_delays: Vec<Duration>,
    pub(crate) txs_delays_relative: Vec<Duration>,

    pub(crate) txs_received_outdated: Vec<usize>,
}

impl Display for TxStatCollector {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let total_txs: usize = self.txs_received_first.iter().sum();

        let tx_delays_avg: Vec<i64> = self
            .txs_delays
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if total_txs - self.txs_received_first[i] == 0 {
                    0
                } else {
                    x.num_microseconds().unwrap_or_default() / ((total_txs - self.txs_received_first[i]) as i64)
                }
            })
            .collect();

        let total_txs_rel: usize = self.txs_received_first_relative.iter().sum();

        let tx_delays_relative_avg: Vec<i64> = self
            .txs_delays
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if total_txs_rel - self.txs_received_first_relative[i] == 0 {
                    0
                } else {
                    x.num_microseconds().unwrap_or_default() / ((total_txs_rel - self.txs_received_first_relative[i]) as i64)
                }
            })
            .collect();
        writeln!(
            f,
            "txs total in blocks: {} received by nodes: {} per node {:?}  outdated {:?}",
            self.total_txs, self.total_received_tx, self.txs_received, self.txs_received_outdated,
        )?;
        writeln!(f, "txs abs first {:?} delays avg {:?} μs", self.txs_received_first, tx_delays_avg)?;
        writeln!(f, "txs rel first {:?} delays avg {:?} μs", self.txs_received_first_relative, tx_delays_relative_avg)?;

        Ok(())
    }
}

impl TxStatCollector {
    pub fn new(nodes_count: usize) -> TxStatCollector {
        TxStatCollector {
            txs_received: vec![0; nodes_count],
            txs_received_first: vec![0; nodes_count],
            txs_received_first_relative: vec![0; nodes_count],
            txs_delays: vec![Duration::default(); nodes_count],
            txs_delays_relative: vec![Duration::default(); nodes_count],

            txs_received_outdated: vec![0; nodes_count],
            ..TxStatCollector::default()
        }
    }
}

async fn collect_stat_task(
    id: usize,
    provider: RootProvider,
    grps: bool,
    stat: Arc<RwLock<StatCollector>>,
    warn_up_blocks: usize,
    blocks_needed: usize,
    ping_time: TimeDelta,
) -> Result<()> {
    let bc = Blockchain::new(1);

    let bc_state = BlockchainState::<LoomDB>::new();
    let strategy = Strategy::<LoomDB>::new();

    let encoder = MulticallerSwapEncoder::default();

    let mut bc_actors = BlockchainActors::new(provider, encoder, bc.clone(), bc_state, strategy, vec![]);
    if grps {
        bc_actors.with_exex_events()?;
    } else {
        bc_actors.with_block_events(NodeBlockActorConfig::all_enabled())?.with_local_mempool_events()?;
    }

    let mut blocks_counter: usize = 0;

    let mut block_header_subscription = bc.new_block_headers_channel().subscribe().await;
    let mut block_with_tx_subscription = bc.new_block_with_tx_channel().subscribe().await;
    let mut block_logs_subscription = bc.new_block_logs_channel().subscribe().await;
    let mut block_state_subscription = bc.new_block_state_update_channel().subscribe().await;

    let mut pending_tx_subscription = bc.mempool_events_channel().subscribe().await;

    loop {
        select! {
            header = block_header_subscription.recv() => {
                match header {
                    Ok(header)=>{
                        let block_number = header.inner.header.number;
                        let block_hash = header.inner.header.hash;
                        stat.write().await.blocks.insert(block_hash, block_number);

                        blocks_counter += 1;
                        if blocks_counter >= warn_up_blocks {
                            let recv_time = stat.write().await.block_headers.entry(block_number).or_default().add_now(id);
                            println!("{id} : {} block header received {} {}", block_number, block_hash, recv_time - ping_time);
                        }else{
                            println!("Warming up {id} : {} block header received {}", block_number, block_hash);
                        }

                        if blocks_counter >= blocks_needed + warn_up_blocks {
                            break;
                        }
                    }
                    Err(e)=>{
                        println!("Error receiving block header {id} {e}");
                    }
                }

            }
            block_msg = block_with_tx_subscription.recv() => {
                match block_msg {
                    Ok(block_msg)=>{
                        let block_number = block_msg.block.header.number;
                        let block_hash = block_msg.block.header.hash;
                        if blocks_counter >= warn_up_blocks {
                            let recv_time = stat.write().await.block_with_tx.entry(block_number).or_default().add_now(id);
                            println!("{id} : {} block with tx received {} {}", block_number, block_hash, recv_time - ping_time);
                        }else{
                            println!("Warming up {id} : {} block with tx received {}", block_number, block_hash);
                        }
                    }
                    Err(e)=>{
                        println!("Error receiving block with tx {id} {e}");
                    }
                }
            }
            logs = block_logs_subscription.recv() => {
                match logs {
                    Ok(logs)=>{
                        let block_number = stat.read().await.blocks.get(&logs.block_header.hash).cloned().unwrap_or_default();

                        if blocks_counter >= warn_up_blocks {
                            let recv_time = stat.write().await.block_logs.entry(block_number).or_default().add_now(id);
                            println!("{id} : {} block logs received {} {}", block_number, logs.block_header.hash, recv_time - ping_time);
                        }else{
                            println!("Warming up {id} : {} block logs received {}", block_number, logs.block_header.hash);
                        }
                    }
                    Err(e)=>{
                        println!("Error receiving block logs {id} {e}");
                    }
                }
            }


            state_update = block_state_subscription.recv() => {
                match state_update  {
                    Ok(state_update)=>{
                        let block_number = stat.read().await.blocks.get(&state_update.block_header.hash).cloned().unwrap_or_default();
                        let block_hash = state_update.block_header.hash;

                        if blocks_counter >= warn_up_blocks {
                            let recv_time = stat.write().await.block_state.entry(block_number).or_default().add_now(id);
                            println!("{id} : {} block state received {} {}", block_number, block_hash, recv_time - ping_time);
                        }else{
                            println!("Warming up {id} : {} block state tx received {}", block_number, block_hash);
                        }
                    }
                    Err(e)=>{
                        println!("Error receiving block state {id} {e}");
                    }
                }
            }

            mempool_event = pending_tx_subscription.recv() =>{
                match mempool_event {
                    Ok(mempool_event) =>{
                        if let MempoolEvents::MempoolTxUpdate{ tx_hash} = mempool_event {
                            stat.write().await.txs.entry(tx_hash).or_default().add_now(id);
                        }
                    }
                    Err(e)=>{
                        println!("Error receiving tx {id} {e}");
                    }
                }

            }

        }
    }
    println!("{id} finished");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug,alloy_rpc_client=info,h2=info"));
    let cli = Cli::parse();

    if cli.endpoint.is_empty() {
        return Err(eyre!("NO_NODES_SELECTED"));
    }

    let nodes_count = cli.endpoint.len();

    let stat = Arc::new(RwLock::new(StatCollector::default()));

    println!("Hello, nodebench!");

    let mut tasks: Vec<JoinHandle<_>> = vec![];

    let mut first_provider: Option<RootProvider> = None;
    let mut prev_provider: Option<RootProvider> = None;

    for (idx, endpoint) in cli.endpoint.iter().enumerate() {
        //let conn = WsConnect::new(endpoint.clone());
        let (provider, is_grpc) = if endpoint == "grpc" {
            (prev_provider.clone().unwrap(), true)
        } else {
            (ProviderBuilder::new().disable_recommended_fillers().on_builtin(endpoint.clone().as_str()).await?, false)
        };

        prev_provider = Some(provider.clone());

        if first_provider.is_none() {
            first_provider = Some(provider.clone());
        }

        let start_time = Local::now();
        for _i in 0u64..10 {
            let block_number = provider.get_block_number().await?;
            let _ = provider.get_block_by_number(BlockNumberOrTag::Number(block_number), BlockTransactionsKind::Hashes).await?;
        }
        let ping_time = (Local::now() - start_time) / (10 * 2);
        println!("Ping time {idx} : {ping_time}");
        stat.write().await.ping.push(ping_time);

        let join_handler = tokio::spawn(collect_stat_task(idx, provider, is_grpc, stat.clone(), 3, 10, ping_time));
        tasks.push(join_handler);
    }

    join_all(tasks).await;

    let stat = stat.read().await;
    let first_provider = first_provider.unwrap();

    let mut calc = TxStatCollector::new(cli.endpoint.len());

    println!("{}", stat);

    for (block_number, _) in stat.block_headers.iter() {
        println!("Getting block {block_number}");
        let block =
            first_provider.get_block_by_number(BlockNumberOrTag::Number(*block_number), BlockTransactionsKind::Hashes).await?.unwrap();

        calc.total_txs += block.transactions.len();

        let block_time_map = stat.block_headers.get(block_number).unwrap();

        if let BlockTransactions::Hashes(tx_hash_vec) = block.transactions {
            for tx_hash in tx_hash_vec {
                if let Some(tx_time) = stat.txs.get(&tx_hash) {
                    calc.total_received_tx += 1;
                    for node_id in 0..nodes_count {
                        let block_time_node = block_time_map.get_time(node_id).unwrap();

                        if let Some(tx_local_time) = tx_time.get_time(node_id) {
                            calc.txs_received[node_id] += 1;

                            // check if tx received after block
                            if tx_local_time > block_time_node
                                || tx_time.get_time_delta(node_id).unwrap_or_default() > TimeDelta::seconds(2)
                            {
                                calc.txs_received_outdated[node_id] += 1;
                            } else {
                                // calc absolute delay
                                if let Some(t) = tx_time.get_time_delta(node_id) {
                                    calc.txs_delays[node_id] += t;
                                    if t.is_zero() {
                                        calc.txs_received_first[node_id] += 1;
                                    }
                                }
                                //calc relative delay
                                if let Some(t) = tx_time.to_relative(&stat.ping).get_time_delta(node_id) {
                                    calc.txs_delays_relative[node_id] += t;
                                    if t.is_zero() {
                                        calc.txs_received_first_relative[node_id] += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("{calc}");

    Ok(())
}
