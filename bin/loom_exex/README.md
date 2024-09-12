# Loom ExEx

Fast implementation of backrun bot as ExEx module

Bot is constructed with BlockchainActors in the following way:
Config file is still required for Multicaller address

```rust

let mut bc_actors = BlockchainActors::new(provider.clone(), bc.clone(), vec![]);
    bc_actors
        .mempool().await?

        .initialize_signers_with_encrypted_key(private_key_encrypted).await? // initialize signer with encrypted key
        .with_block_history().await? // collect blocks
        .with_health_monitor_pools().await? // monitor pools health to disable empty
        .with_health_monitor_state().await? // monitor state health
        .with_health_monitor_stuffing_tx().await? // collect stuffing tx information
        .with_encoder(multicaller_address).await? // convert swaps to opcodes and passes to estimator
        .with_evm_estimator().await? // estimate gas, add tips
        .with_signers().await? // start signer actor that signs transactions before broadcasting
        .with_flashbots_broadcaster(true).await? // broadcast signed txes to flashbots
        .with_market_state_preloader().await? // preload contracts to market state
        .with_nonce_and_balance_monitor().await? // start monitoring balances of
        .with_pool_history_loader().await? // load pools used in latest 10000 blocks
        .with_pool_protocol_loader().await? // load curve + steth + wsteth
        .with_new_pool_loader().await? // load new pools // TODO : fix subscription
        .with_swap_path_merger().await? // load merger for multiple swap paths
        .with_diff_path_merger().await? // load merger for different swap paths
        .with_same_path_merger().await? // load merger for same swap paths with different stuffing txes
        .with_backrun_block().await? // load backrun searcher for incoming block
        .with_backrun_mempool().await? // load backrun searcher for mempool txes
    ;


    bc_actors.wait().await;

```



