use alloy::sol_types::private::U256;
use alloy_network::Network;
use alloy_primitives::Address;
use alloy_provider::Provider;
use alloy_transport::Transport;
use eyre::eyre;
use log::{debug, error};

use debug_provider::{AnvilProviderExt, DebugProviderExt};
use defi_abi::IERC20::IERC20Instance;
use defi_entities::NWETH;
use loom_utils::remv_db_direct_access::calc_hashmap_cell;

#[allow(dead_code)]
pub async fn preset_balances<P, T, N>(client: P, target_address: Address, token_address: Address) -> eyre::Result<()>
where
    N: Network,
    T: Transport + Clone,
    P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let balance_storage_cell = calc_hashmap_cell(U256::from(3u32), U256::from_be_slice(target_address.as_slice()));

    let value = client.get_storage_at(token_address, balance_storage_cell).await?;

    if value.is_zero() {
        Err(eyre!("BAD_BALANCE_CELL"))
    } else {
        debug!("Balance at cell balance_storage_cell {balance_storage_cell} is {value}");
        Ok(())
    }
}

pub async fn set_balance<P, T, N>(client: P, target_address: Address, token_address: Address) -> eyre::Result<()>
where
    T: Transport + Clone,
    N: alloy_network::Network,
    P: Provider<T, N> + AnvilProviderExt<T, N> + Send + Sync + Clone + 'static,
{
    let weth_balance = NWETH::from_float(1.0);

    let balance_cell = calc_hashmap_cell(U256::from(3), U256::from_be_slice(target_address.as_slice()));

    match client.set_storage(token_address, balance_cell.into(), weth_balance.into()).await {
        Err(e) => {
            error!("{e}");
            return Err(eyre!(e));
        }
        _ => {}
    }

    let new_storage = client.get_storage_at(token_address, balance_cell).await?;

    if weth_balance != new_storage {
        error!("{weth_balance} != {new_storage}");
        return Err(eyre!("STORAGE_NOT_SET"));
    }

    let weth_instance = IERC20Instance::new(token_address, client.clone());

    let balance = weth_instance.balanceOf(target_address).call().await?;
    if balance._0 != NWETH::from_float(1.0) {
        return Err(eyre!("BALANCE_NOT_SET"));
    }
    Ok(())
}
