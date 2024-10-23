use crate::remv_db_direct_access::calc_hashmap_cell;
use crate::{nweth, NWETH};
use alloy::{network::Network, primitives::Address, providers::Provider, sol_types::private::U256, transports::Transport};
use debug_provider::{AnvilProviderExt, DebugProviderExt};
use defi_abi::IERC20::IERC20Instance;
use eyre::{eyre, Result};
use loom_revm_db::LoomDBType;
use tracing::error;

pub struct BalanceCheater {}

#[allow(dead_code)]
impl BalanceCheater {
    pub fn get_balance_cell(token: Address, owner: Address) -> Result<U256> {
        match token {
            NWETH::ADDRESS => Ok(calc_hashmap_cell(U256::from(3u32), U256::from_be_slice(owner.as_slice()))),
            _ => Err(eyre!("ADDRESS_CELL_UNKNOWN")),
        }
    }

    pub async fn get_anvil_token_balance<P, T, N>(client: P, token: Address, owner: Address) -> eyre::Result<U256>
    where
        N: Network,
        T: Transport + Clone,
        P: Provider<T, N> + DebugProviderExt<T, N> + Send + Sync + Clone + 'static,
    {
        let value = client.get_storage_at(token, Self::get_balance_cell(token, owner)?).await?;

        Ok(value)
    }

    pub async fn set_anvil_token_balance<P, T, N>(client: P, token: Address, owner: Address, balance: U256) -> eyre::Result<()>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + AnvilProviderExt<T, N> + Send + Sync + Clone + 'static,
    {
        let balance_cell = Self::get_balance_cell(token, owner)?;

        if let Err(e) = client.set_storage(token, balance_cell.into(), balance.into()).await {
            error!("{e}");
            return Err(eyre!(e));
        }

        let new_storage = client.get_storage_at(token, balance_cell).await?;

        if balance != new_storage {
            error!("{balance} != {new_storage}");
            return Err(eyre!("STORAGE_NOT_SET"));
        }

        let weth_instance = IERC20Instance::new(token, client.clone());

        let balance = weth_instance.balanceOf(owner).call().await?;
        if balance._0 != nweth::NWETH::from_float(1.0) {
            return Err(eyre!("BALANCE_NOT_SET"));
        }
        Ok(())
    }
    pub async fn set_anvil_token_balance_float<P, T, N>(client: P, token: Address, owner: Address, balance: f64) -> eyre::Result<()>
    where
        T: Transport + Clone,
        N: Network,
        P: Provider<T, N> + AnvilProviderExt<T, N> + Send + Sync + Clone + 'static,
    {
        let balance = nweth::NWETH::from_float(balance);
        Self::set_anvil_token_balance(client, token, owner, balance).await
    }

    pub fn set_evm_token_balance(db: &mut LoomDBType, token: Address, owner: Address, balance: U256) -> eyre::Result<()> {
        let balance_cell = calc_hashmap_cell(U256::from(3), U256::from_be_slice(owner.as_slice()));

        db.insert_account_storage(token, balance_cell, balance).map_err(|_| eyre!("ERROR_INSERTING_ACCOUNT_STORAGE"))
    }

    pub fn set_evm_token_balance_float(db: &mut LoomDBType, token: Address, owner: Address, balance: f64) -> eyre::Result<()> {
        Self::set_evm_token_balance(db, token, owner, NWETH::from_float(balance))
    }
}
