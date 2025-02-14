use crate::remv_db_direct_access::calc_hashmap_cell;
use crate::{nweth, NWETH};
use alloy::providers::ext::AnvilApi;
use alloy::{network::Network, primitives::Address, providers::Provider, sol_types::private::U256};
use eyre::{eyre, Result};
use loom_defi_abi::IERC20::IERC20Instance;
use loom_defi_address_book::TokenAddressEth;
use loom_evm_db::LoomDBType;
use loom_node_debug_provider::DebugProviderExt;
use tracing::error;

pub struct BalanceCheater {}

#[allow(dead_code)]
impl BalanceCheater {
    pub fn get_balance_cell(token: Address, owner: Address) -> Result<U256> {
        match token {
            TokenAddressEth::WETH => Ok(calc_hashmap_cell(U256::from(3u32), U256::from_be_slice(owner.as_slice()))),
            TokenAddressEth::USDT => Ok(calc_hashmap_cell(U256::from(2u32), U256::from_be_slice(owner.as_slice()))),
            TokenAddressEth::USDC => Ok(calc_hashmap_cell(U256::from(3u32), U256::from_be_slice(owner.as_slice()))),
            TokenAddressEth::WSTETH => Ok(calc_hashmap_cell(U256::from(0u32), U256::from_be_slice(owner.as_slice()))),
            TokenAddressEth::STETH => Ok(calc_hashmap_cell(U256::from(3u32), U256::from_be_slice(owner.as_slice()))),
            _ => Err(eyre!("ADDRESS_CELL_UNKNOWN")),
        }
    }

    pub async fn get_anvil_token_balance<P, N>(client: P, token: Address, owner: Address) -> eyre::Result<U256>
    where
        N: Network,
        P: Provider<N> + DebugProviderExt<N> + Send + Sync + Clone + 'static,
    {
        let value = client.get_storage_at(token, Self::get_balance_cell(token, owner)?).await?;

        Ok(value)
    }

    pub async fn set_anvil_token_balance<P, N>(client: P, token: Address, owner: Address, balance: U256) -> eyre::Result<()>
    where
        N: Network,
        P: Provider<N> + Send + Sync + Clone + 'static,
    {
        let balance_cell = Self::get_balance_cell(token, owner)?;

        if let Err(e) = client.anvil_set_storage_at(token, balance_cell, balance.into()).await {
            error!("{e}");
            return Err(eyre!(e));
        }

        let new_storage = client.get_storage_at(token, balance_cell).await?;

        if balance != new_storage {
            error!("{balance} != {new_storage}");
            return Err(eyre!("STORAGE_NOT_SET"));
        }

        let token_instance = IERC20Instance::new(token, client.clone());

        let new_balance = token_instance.balanceOf(owner).call_raw().await?;
        println!("new_balance : {:?}", new_balance);
        if U256::from_be_slice(new_balance.as_ref()) != balance {
            return Err(eyre!("BALANCE_NOT_SET"));
        }
        Ok(())
    }
    pub async fn set_anvil_token_balance_float<P, N>(client: P, token: Address, owner: Address, balance: f64) -> eyre::Result<()>
    where
        N: Network,
        P: Provider<N> + Send + Sync + Clone + 'static,
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
