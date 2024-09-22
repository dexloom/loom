#[cfg(test)]
mod test {
    use crate::BlockHistoryActor;
    use alloy_network::Ethereum;
    use alloy_provider::Provider;
    use alloy_transport::Transport;
    use defi_blockchain::Blockchain;
    use defi_events::{BlockLogs, BlockStateUpdate, MessageBlockHeader};
    use log::error;
    use revm::db::DatabaseRef;

    use alloy_eips::BlockNumberOrTag;
    use alloy_node_bindings::Anvil;
    use alloy_primitives::{Address, BlockHash, B256, U256};
    use alloy_provider::ext::AnvilApi;
    use alloy_provider::ProviderBuilder;
    use alloy_rpc_client::ClientBuilder;
    use alloy_rpc_types::{Block, Filter, Header, Log};
    use debug_provider::AnvilProviderExt;
    use defi_events::{BlockHeader, Message};
    use defi_types::{ChainParameters, GethStateUpdate, GethStateUpdateVec};
    use eyre::eyre;
    use log::info;
    use loom_actors::Actor;
    use loom_revm_db::LoomInMemoryDB;
    use loom_utils::geth_state_update::{account_state_add_storage, account_state_with_nonce_and_balance, geth_state_update_add_account};
    use revm::Database;
    use std::time::Duration;

    async fn broadcast_to_channels(
        bc: &Blockchain,
        header: Option<Header>,
        block: Option<Block>,
        logs: Option<Vec<Log>>,
        state_update: Option<GethStateUpdateVec>,
    ) -> eyre::Result<()> {
        let chain_params = ChainParameters::ethereum();
        let mut block_hash = BlockHash::ZERO;

        if let Some(header) = header {
            block_hash = header.hash;
            let header_msg: MessageBlockHeader = Message::new(BlockHeader::new(chain_params.clone(), header));
            if let Err(e) = bc.new_block_headers_channel().send(header_msg).await {
                error!("bc.new_block_headers_channel().send : {}", e)
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        if let Some(block) = block {
            block_hash = block.header.hash;

            if let Err(e) = bc.new_block_with_tx_channel().send(block).await {
                error!("bc.new_block_with_tx_channel().send : {}", e)
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        if let Some(logs) = logs {
            let logs_msg = BlockLogs { block_hash, logs };

            if let Err(e) = bc.new_block_logs_channel().send(logs_msg).await {
                error!("bc.new_block_with_tx_channel().send : {}", e)
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        if let Some(state_update) = state_update {
            let state_update_msg = BlockStateUpdate { block_hash, state_update };
            if let Err(e) = bc.new_block_state_update_channel().send(state_update_msg).await {
                error!("bc.new_block_with_tx_channel().send : {}", e)
            }
        }

        Ok(())
    }

    async fn broadcast_latest_block<P, T>(provider: P, bc: &Blockchain, state_update: Option<GethStateUpdateVec>) -> eyre::Result<()>
    where
        T: Transport + Send + Sync + Clone + 'static,
        P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        let block = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await?.unwrap();
        let filter = Filter::new().at_block_hash(block.header.hash);

        let logs = provider.get_logs(&filter).await?;

        let state_update = state_update.unwrap_or_default();

        broadcast_to_channels(bc, Some(block.header.clone()), Some(block), Some(logs), Some(state_update)).await
    }

    async fn test_actor_block_history_actor_chain_head_worker<P, T>(provider: P, bc: Blockchain) -> eyre::Result<()>
    where
        T: Transport + Send + Sync + Clone + 'static,
        P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        const ADDR_01: Address = Address::repeat_byte(1);
        let CELL_01: B256 = B256::from(U256::from_limbs([1, 0, 0, 0]));
        let VALUE_02: B256 = B256::from(U256::from_limbs([2, 0, 0, 0]));
        let VALUE_03: B256 = B256::from(U256::from_limbs([3, 0, 0, 0]));

        let account_1 = account_state_add_storage(account_state_with_nonce_and_balance(1, U256::from(2)), CELL_01, VALUE_02);

        let state_0 = geth_state_update_add_account(GethStateUpdate::default(), ADDR_01, account_1);

        let state_update_0 = vec![state_0];

        let mut db = LoomInMemoryDB::default();
        db.apply_geth_update_vec(state_update_0);

        bc.market_state().write().await.state_db = db;

        let account_01 = bc.market_state().read().await.state_db.clone().load_account(ADDR_01).cloned()?;
        assert_eq!(account_01.info.nonce, 1);
        assert_eq!(account_01.info.balance, U256::from(2));
        for (k, v) in account_01.storage.iter() {
            print!("{} {}", k, v)
        }
        let state_1 =
            geth_state_update_add_account(GethStateUpdate::default(), ADDR_01, account_state_with_nonce_and_balance(2, U256::from(3)));

        broadcast_latest_block(provider.clone(), &bc, Some(vec![state_1])).await?; // block 0

        // Check state after first block update
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let account_01 = bc.market_state().read().await.state_db.clone().load_account(ADDR_01).cloned()?;
        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().nonce, 2);
        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().balance, U256::from(3));
        assert_eq!(bc.market_state().read().await.state_db.storage_ref(ADDR_01, U256::from(1))?, U256::from(2));

        let snap = provider.anvil_snapshot().await?;
        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 1#0
        broadcast_latest_block(provider.clone(), &bc, None).await?; // broadcast 1#0

        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 2#0
        let block_2_0 = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await?.unwrap();

        broadcast_latest_block(provider.clone(), &bc, None).await?; // broadcast 2#0

        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 3#0
        let block_3_0 = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await?.unwrap();

        provider.anvil_revert(snap).await?;
        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 1#1

        let account_1_1 = account_state_add_storage(account_state_with_nonce_and_balance(4, U256::from(5)), CELL_01, VALUE_03);
        let state_1_1 = geth_state_update_add_account(GethStateUpdate::default(), ADDR_01, account_1_1);
        broadcast_latest_block(provider.clone(), &bc, Some(vec![state_1_1])).await?; // broadcast 1#1

        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().nonce, 2);
        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().balance, U256::from(3));
        assert_eq!(bc.market_state().read().await.state_db.storage_ref(ADDR_01, U256::from(1))?, U256::from(2));

        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 2#1
        let block_2_1 = provider.get_block_by_number(BlockNumberOrTag::Latest, true).await?.unwrap();

        broadcast_latest_block(provider.clone(), &bc, None).await?; // broadcast 2#1, chain_head must change

        tokio::time::sleep(Duration::from_millis(1000)).await;

        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().nonce, 4);
        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().balance, U256::from(5));
        assert_eq!(bc.market_state().read().await.state_db.storage_ref(ADDR_01, U256::from(1))?, U256::from(3));

        assert_eq!(bc.latest_block().read().await.block_hash, block_2_1.header.hash);
        assert_eq!(bc.block_history().read().await.latest_block_number, block_2_1.header.number);
        assert_eq!(
            bc.block_history().read().await.get_block_hash_for_block_number(block_2_1.header.number).unwrap(),
            block_2_1.header.hash
        );

        broadcast_to_channels(&bc, Some(block_3_0.header.clone()), Some(block_3_0.clone()), Some(vec![]), Some(vec![])).await?; // broadcast 3#0, chain_head must change

        assert_eq!(bc.latest_block().read().await.block_hash, block_3_0.header.hash);
        assert_eq!(bc.block_history().read().await.latest_block_number, block_3_0.header.number);
        assert_eq!(
            bc.block_history().read().await.get_block_hash_for_block_number(block_3_0.header.number).unwrap(),
            block_3_0.header.hash
        );
        assert_eq!(
            bc.block_history().read().await.get_block_hash_for_block_number(block_2_0.header.number).unwrap(),
            block_2_0.header.hash
        );
        tokio::time::sleep(Duration::from_millis(1000)).await;

        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().nonce, 2);
        assert_eq!(bc.market_state().read().await.state_db.basic_ref(ADDR_01)?.unwrap().balance, U256::from(3));
        assert_eq!(bc.market_state().read().await.state_db.storage_ref(ADDR_01, U256::from(1))?, U256::from(2));

        Ok(())
    }

    #[tokio::test]
    async fn test_actor_block_history_actor_chain_head() -> eyre::Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or(
            "debug,defi_entities::block_history=trace,tokio_tungstenite=off,tungstenite=off,hyper_util=off,alloy_transport_http=off",
        ));

        let anvil = Anvil::new().try_spawn()?;
        let client_anvil = ClientBuilder::default().http(anvil.endpoint_url()).boxed();
        let provider = ProviderBuilder::new().on_client(client_anvil);

        let blockchain = Blockchain::new(1);
        BlockHistoryActor::new(provider.clone()).on_bc(&blockchain).start()?;
        let bc = blockchain.clone();
        tokio::task::spawn(async move {
            if let Err(e) = test_actor_block_history_actor_chain_head_worker(provider.clone(), bc).await {
                error!("test_worker : {}", e);
            } else {
                info!("test_worker finished");
            }
        });

        let mut rx = blockchain.market_events_channel().subscribe().await;
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        _=>{
                            info!("{:?}", msg)
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(10000)) => {
                    break;
                }
            }
        }

        //let block_history = blockchain.block_history().clone();
        //let block_history = block_history.read().await;
        //assert_eq!(block_history.len(), 6);

        Ok(())
    }

    async fn test_actor_block_history_actor_reorg_worker<P, T>(provider: P, bc: Blockchain) -> eyre::Result<()>
    where
        T: Transport + Send + Sync + Clone + 'static,
        P: Provider<T, Ethereum> + Send + Sync + Clone + 'static,
    {
        let snap = provider.anvil_snapshot().await?;

        broadcast_latest_block(provider.clone(), &bc, None).await?; // block 0
        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 1#0
        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 2#0
        broadcast_latest_block(provider.clone(), &bc, None).await?; // block 2#0

        provider.anvil_revert(snap).await?;

        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 1#1
        broadcast_latest_block(provider.clone(), &bc, None).await?;
        provider.anvil_mine(Some(U256::from(1)), None).await?; // mine block 2#1
        broadcast_latest_block(provider.clone(), &bc, None).await?;
        provider.anvil_mine(Some(U256::from(1)), None).await.map_err(|_| eyre!("3#1"))?; // mine block 3#1
        broadcast_latest_block(provider.clone(), &bc, None).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_actor_block_history_actor_reorg() -> eyre::Result<()> {
        let _ = env_logger::try_init_from_env(env_logger::Env::default().default_filter_or("info,tokio_tungstenite=off,tungstenite=off"));

        let anvil = Anvil::new().try_spawn()?;
        let client_anvil = ClientBuilder::default().http(anvil.endpoint_url()).boxed();

        let provider = ProviderBuilder::new().on_client(client_anvil);

        let blockchain = Blockchain::new(1);
        BlockHistoryActor::new(provider.clone()).on_bc(&blockchain).start()?;

        let bc = blockchain.clone();
        tokio::task::spawn(async move {
            if let Err(e) = test_actor_block_history_actor_reorg_worker(provider.clone(), bc).await {
                error!("test_worker : {}", e);
            } else {
                info!("test_worker finished");
            }
        });

        let mut rx = blockchain.market_events_channel().subscribe().await;
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        _=>{
                            info!("{:?}", msg)
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(1000)) => {
                    break;
                }
            }
        }

        let block_history = blockchain.block_history().clone();
        let block_history = block_history.read().await;
        assert_eq!(block_history.len(), 6);

        Ok(())
    }
}
