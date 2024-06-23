use std::{collections::HashMap, fmt::Display, sync::Arc};

use alloy::{
    eips::BlockNumberOrTag,
    primitives::TxHash,
    providers::{Provider, ProviderBuilder, RootProvider, WsConnect},
    pubsub::PubSubFrontend,
    rpc::types::{Block, BlockTransactions, Transaction},
};
use chrono::{DateTime, Duration, Local, TimeDelta};
use clap::Parser;
use eyre::Result;
use futures::future::join_all;
use tokio::{select, sync::RwLock, task::JoinHandle};

use crate::cli::Cli;

mod cli;

#[derive(Clone, Debug, Default)]
pub struct TimeMap {
    time: HashMap<usize, DateTime<Local>>,
}

impl TimeMap {
    pub fn add_time(&mut self, id: usize, time: DateTime<Local>) {
        self.time.entry(id).or_insert(time);
    }
    pub fn add_now(&mut self, id: usize) -> DateTime<Local> {
        *self.time.entry(id).or_insert(Local::now())
    }

    pub fn get_time(&self, id: usize) -> Option<&DateTime<Local>> {
        self.time.get(&id)
    }

    pub fn to_relative(&self, pings: &Vec<TimeDelta>) -> TimeMap {
        let rel_time: HashMap<usize, DateTime<Local>> = self
            .time
            .iter()
            .map(|(k, v)| (*k, *v - pings.get(*k).cloned().unwrap()))
            .collect();
        TimeMap { time: rel_time }
    }

    pub fn get_first_time(&self) -> DateTime<Local> {
        self.time.values().min().cloned().unwrap_or_default()
    }

    pub fn get_time_delta(&self, id: usize) -> Option<TimeDelta> {
        self.time.get(&id).map(|x| *x - self.get_first_time())
    }
}

#[derive(Clone, Debug, Default)]
pub struct StatCollector {
    ping: Vec<TimeDelta>,
    blocks: HashMap<u64, TimeMap>,
    txs: HashMap<TxHash, TimeMap>,
}

#[derive(Clone, Debug, Default)]
pub struct StatCalculator {
    pub(crate) blocks_received_first: Vec<usize>,
    pub(crate) blocks_received_first_relative: Vec<usize>,
    pub(crate) blocks_delays: Vec<Duration>,
    pub(crate) blocks_delays_relative: Vec<Duration>,

    pub(crate) total_received_tx: usize,
    pub(crate) total_txs: usize,
    pub(crate) txs_received: Vec<usize>,
    pub(crate) txs_received_first: Vec<usize>,
    pub(crate) txs_received_first_relative: Vec<usize>,
    pub(crate) txs_delays: Vec<Duration>,
    pub(crate) txs_delays_relative: Vec<Duration>,

    pub(crate) txs_received_outdated: Vec<usize>,
}

impl Display for StatCalculator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_block: usize = self.blocks_received_first.iter().sum();
        let block_delays_avg: Vec<i64> = self
            .blocks_delays
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if total_block - self.blocks_received_first[i] == 0 {
                    0
                } else {
                    x.num_milliseconds() / ((total_block - self.blocks_received_first[i]) as i64)
                }
            })
            .collect();

        let block_delays_rel: Vec<i64> = self
            .blocks_delays_relative
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if total_block - self.blocks_received_first_relative[i] == 0 {
                    0
                } else {
                    x.num_milliseconds()
                        / ((total_block - self.blocks_received_first_relative[i]) as i64)
                }
            })
            .collect();

        let total_txs: usize = self.txs_received_first.iter().sum();

        let tx_delays_avg: Vec<i64> = self
            .txs_delays
            .iter()
            .enumerate()
            .map(|(i, x)| {
                if total_txs - self.txs_received_first[i] == 0 {
                    0
                } else {
                    x.num_milliseconds() / ((total_txs - self.txs_received_first[i]) as i64)
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
                    x.num_milliseconds()
                        / ((total_txs_rel - self.txs_received_first_relative[i]) as i64)
                }
            })
            .collect();
        writeln!(
            f,
            "blocks abs first {:?} avg ms {:?}",
            self.blocks_received_first, block_delays_avg,
        )?;

        writeln!(
            f,
            "blocks rel first {:?} avg ms {:?}",
            self.blocks_received_first_relative, block_delays_rel,
        )?;
        writeln!(
            f,
            "txs total : {} received by nodes  {:?} total {}, outdated {:?}",
            self.total_txs, self.txs_received, self.total_received_tx, self.txs_received_outdated,
        )?;
        writeln!(
            f,
            "txs abs first {:?} delays avg ms {:?}",
            self.txs_received_first, tx_delays_avg
        )?;
        writeln!(
            f,
            "txs rel first {:?} delays avg ms {:?}",
            self.txs_received_first_relative, tx_delays_relative_avg
        )?;

        Ok(())
    }
}

impl StatCalculator {
    pub fn new(nodes_count: usize) -> StatCalculator {
        StatCalculator {
            blocks_received_first: vec![0; nodes_count],
            blocks_received_first_relative: vec![0; nodes_count],
            blocks_delays: vec![Duration::default(); nodes_count],
            blocks_delays_relative: vec![Duration::default(); nodes_count],

            txs_received: vec![0; nodes_count],
            txs_received_first: vec![0; nodes_count],
            txs_received_first_relative: vec![0; nodes_count],
            txs_delays: vec![Duration::default(); nodes_count],
            txs_delays_relative: vec![Duration::default(); nodes_count],

            txs_received_outdated: vec![0; nodes_count],
            ..StatCalculator::default()
        }
    }
}

async fn collect_stat_task(
    id: usize,
    provider: RootProvider<PubSubFrontend>,
    stat: Arc<RwLock<StatCollector>>,
    warn_up_blocks: usize,
    blocks_needed: usize,
    ping_time: TimeDelta,
) -> Result<()> {
    let mut block_subscription = provider.subscribe_blocks().await?;
    let mut pending_tx_subscription = provider.subscribe_full_pending_transactions().await?;

    let mut blocks_counter: usize = 0;

    loop {
        select! {
            block = block_subscription.recv() => {
                match  block {
                    Ok(block)=>{
                        let block_number = block.header.number.unwrap_or_default();
                        let block_hash = block.header.hash.unwrap_or_default();
                        if blocks_counter >= warn_up_blocks {
                            let recv_time = stat.write().await.blocks.entry(block_number).or_default().add_now(id);
                            println!("{id} : {} block received {} {}", block_number, block_hash, recv_time - ping_time);
                        }else{
                            println!("Warmign up {id} : {} block received {}", block_number, block_hash);
                        }

                        blocks_counter += 1;
                        if blocks_counter >= blocks_needed + warn_up_blocks {
                            break;
                        }
                    }
                    Err(e)=>{
                        println!("Error receiving block {id} {e}");
                    }
                }

            }
            tx = pending_tx_subscription.recv() =>{
                match tx {
                    Ok(tx) =>{
                        let tx_hash = tx.hash;
                        stat.write().await.txs.entry(tx_hash).or_default().add_now(id);
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
    let cli = Cli::parse();

    let nodes_count = cli.endpoint.len();

    let stat = Arc::new(RwLock::new(StatCollector::default()));

    println!("Hello, nodebench!");

    let mut tasks: Vec<JoinHandle<_>> = vec![];

    let mut first_provider: Option<RootProvider<PubSubFrontend>> = None;

    for (idx, endpoint) in cli.endpoint.iter().enumerate() {
        let conn = WsConnect::new(endpoint.clone());
        let provider = ProviderBuilder::new().on_ws(conn).await?;

        if first_provider.is_none() {
            first_provider = Some(provider.clone());
        }

        let start_time = Local::now();
        for i in 0u64..10 {
            let block_number = provider.get_block_number().await?;
            let _ = provider
                .get_block_by_number(BlockNumberOrTag::Number(block_number), false)
                .await?;
        }
        let ping_time = (Local::now() - start_time) / (10 * 2);
        println!("Ping time {idx} : {ping_time}");
        stat.write().await.ping.push(ping_time);

        let join_handler = tokio::spawn(collect_stat_task(
            idx,
            provider,
            stat.clone(),
            3,
            10,
            ping_time,
        ));
        tasks.push(join_handler);
    }

    join_all(tasks).await;

    let stat = stat.read().await;
    let first_provider = first_provider.unwrap();

    let mut calc = StatCalculator::new(cli.endpoint.len());

    for (block_number, _) in stat.blocks.iter() {
        println!("Getting block {block_number}");
        let block = first_provider
            .get_block_by_number(BlockNumberOrTag::Number(*block_number), false)
            .await?
            .unwrap();

        calc.total_txs += block.transactions.len();

        let block_time = stat.blocks.get(block_number).unwrap();
        for node_id in 0..nodes_count {
            if let Some(t) = block_time.get_time_delta(node_id) {
                calc.blocks_delays[node_id] += t;
                if t.is_zero() {
                    calc.blocks_received_first[node_id] += 1;
                }
            }

            if let Some(t) = block_time.to_relative(&stat.ping).get_time_delta(node_id) {
                calc.blocks_delays_relative[node_id] += t;
                if t.is_zero() {
                    calc.blocks_received_first_relative[node_id] += 1;
                }
            }
        }

        if let BlockTransactions::Hashes(tx_hash_vec) = block.transactions {
            for tx_hash in tx_hash_vec {
                if let Some(tx_time) = stat.txs.get(&tx_hash) {
                    calc.total_received_tx += 1;
                    for node_id in 0..nodes_count {
                        let block_time_node = block_time.get_time(node_id).unwrap();

                        if let Some(tx_local_time) = tx_time.get_time(node_id) {
                            calc.txs_received[node_id] += 1;

                            // check if tx received after block
                            if tx_local_time > block_time_node
                                || tx_time.get_time_delta(node_id).unwrap_or_default()
                                    > TimeDelta::seconds(2)
                            {
                                calc.txs_received_outdated[node_id] += 1;
                            } else {
                                // calc absolute delay
                                if let Some(t) = tx_time.get_time_delta(node_id) {
                                    //println!("timedelta {node_id} {t}");
                                    calc.txs_delays[node_id] += t;
                                    if t.is_zero() {
                                        calc.txs_received_first[node_id] += 1;
                                    }
                                }
                                //calc relative delay
                                if let Some(t) =
                                    tx_time.to_relative(&stat.ping).get_time_delta(node_id)
                                {
                                    //println!("timedelta relative {node_id} {t}");
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
