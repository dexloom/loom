use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

use alloy::primitives::{address, Address, Bytes, U256};
use alloy::providers::{Network, Provider};
use alloy::rpc::types::{BlockId, BlockNumberOrTag};
use alloy::sol_types::SolInterface;
use eyre::{eyre, Report, Result};
use tracing::{debug, error, trace};

use loom_defi_abi::curve::ICurveAddressProvider::ICurveAddressProviderInstance;
use loom_defi_abi::curve::ICurveCommon::ICurveCommonInstance;
use loom_defi_abi::curve::ICurveCommonI128::ICurveCommonI128Instance;
use loom_defi_abi::curve::ICurveFactory::ICurveFactoryInstance;
use loom_defi_abi::curve::ICurveI128_2::ICurveI128_2Instance;
use loom_defi_abi::curve::ICurveI128_2_To::{ICurveI128_2_ToCalls, ICurveI128_2_ToInstance};
use loom_defi_abi::curve::ICurveI128_2_To_Meta::ICurveI128_2_To_MetaInstance;
use loom_defi_abi::curve::ICurveI128_3::{ICurveI128_3Calls, ICurveI128_3Instance};
use loom_defi_abi::curve::ICurveI128_4::{ICurveI128_4Calls, ICurveI128_4Instance};
use loom_defi_abi::curve::ICurveU256_2::{ICurveU256_2Calls, ICurveU256_2Instance};
use loom_defi_abi::curve::ICurveU256_2_Eth_To::{ICurveU256_2_Eth_ToCalls, ICurveU256_2_Eth_ToInstance};
use loom_defi_abi::curve::ICurveU256_2_To::{ICurveU256_2_ToCalls, ICurveU256_2_ToInstance};
use loom_defi_abi::curve::ICurveU256_3_Eth::{ICurveU256_3_EthCalls, ICurveU256_3_EthInstance};
use loom_defi_abi::curve::ICurveU256_3_Eth_To::{ICurveU256_3_Eth_ToCalls, ICurveU256_3_Eth_ToInstance};
use loom_defi_abi::curve::ICurveU256_3_Eth_To2::{ICurveU256_3_Eth_To2Calls, ICurveU256_3_Eth_To2Instance};
use loom_defi_abi::curve::{
    ICurveI128_2, ICurveI128_2_To, ICurveI128_2_To_Meta, ICurveI128_3, ICurveI128_4, ICurveU256_2, ICurveU256_2_Eth_To, ICurveU256_2_To,
    ICurveU256_3_Eth, ICurveU256_3_Eth_To, ICurveU256_3_Eth_To2,
};

#[derive(Clone, Debug)]
pub enum CurveContract<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    I128_2(ICurveI128_2Instance<(), P, N>),
    I128_2To(ICurveI128_2_ToInstance<(), P, N>),
    I128_2ToMeta(ICurveI128_2_To_MetaInstance<(), P, N>),
    I128_3(ICurveI128_3Instance<(), P, N>),
    I128_4(ICurveI128_4Instance<(), P, N>),
    U256_2(ICurveU256_2Instance<(), P, N>),
    U256_2To(ICurveU256_2_ToInstance<(), P, N>),
    U256_2EthTo(ICurveU256_2_Eth_ToInstance<(), P, N>),
    U256_3Eth(ICurveU256_3_EthInstance<(), P, N>),
    U256_3EthTo(ICurveU256_3_Eth_ToInstance<(), P, N>),
    U256_3EthTo2(ICurveU256_3_Eth_To2Instance<(), P, N>),
}

impl<P, N> Display for CurveContract<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let contract_type = match self {
            CurveContract::I128_2(_) => "I128_2",
            CurveContract::I128_2To(_) => "I128_2_To",
            CurveContract::I128_2ToMeta(_) => "I128_2_To_Meta",
            CurveContract::I128_3(_) => "I128_3",
            CurveContract::I128_4(_) => "I128_4",
            CurveContract::U256_2(_) => "U256_2",
            CurveContract::U256_2To(_) => "U256_2_To",
            CurveContract::U256_2EthTo(_) => "U256_2_Eth_To",
            CurveContract::U256_3Eth(_) => "U256_3_Eth",
            CurveContract::U256_3EthTo(_) => "U256_3_Eth_To",
            CurveContract::U256_3EthTo2(_) => "U256_3_Eth_To2",
            //_ => "CurveUnknown"
        };
        write!(f, "{}", contract_type)
    }
}

pub struct CurveCommonContract<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    _pd: PhantomData<(P, N)>,
}

impl<P, N> CurveCommonContract<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub async fn lp_token(address: Address) -> Result<Address> {
        if address == address!("bEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7") {
            return Ok(address!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490"));
        }
        Err(eyre!("NO_LP_TOKEN"))
    }

    pub async fn coin128(client: P, address: Address, coin_id: u32) -> Result<Address> {
        let common_contract = ICurveCommonI128Instance::new(address, client);
        match common_contract.coins(coin_id.into()).call_raw().await {
            Ok(addr) => {
                if addr.len() >= 32 {
                    Ok(Address::from_slice(&addr[12..32]))
                } else {
                    Err(eyre!("CANNOT_GET_COIN_ADDRESS"))
                }
            }
            Err(e) => {
                trace!("coin call error {}", e);
                Err(eyre!("CANNOT_GET_COIN_ADDRESS"))
            }
        }
    }

    pub async fn coin(client: P, address: Address, coin_id: u32) -> Result<Address> {
        let common_contract = ICurveCommonInstance::new(address, client.clone());
        match common_contract.coins(U256::from(coin_id)).call_raw().await {
            Ok(addr) => {
                if addr.len() >= 32 {
                    Ok(Address::from_slice(&addr[12..32]))
                } else {
                    Self::coin128(client, address, coin_id).await
                }
            }
            Err(e) => {
                trace!("{e}");
                Self::coin128(client, address, coin_id).await
            }
        }
    }

    pub async fn balance128(client: P, address: Address, coin_id: u32) -> Result<U256> {
        let common_contract = ICurveCommonI128Instance::new(address, client);
        match common_contract.balances(coin_id as i128).call_raw().await {
            Ok(ret_data) => {
                if ret_data.len() >= 32 {
                    let balance = U256::from_be_slice(&ret_data[0..32]);
                    Ok(balance)
                } else {
                    Err(eyre!("CANNOT_GET_COIN_BALANCE"))
                }
            }
            Err(e) => {
                trace!("balances128 call error {} : {}", coin_id, e);
                Err(eyre!("CANNOT_GET_COIN_BALANCE"))
            }
        }
    }

    pub async fn balance(client: P, address: Address, coin_id: u32) -> Result<U256> {
        let common_contract = ICurveCommonInstance::new(address, client.clone());
        match common_contract.balances(U256::from(coin_id)).call_raw().await {
            Ok(ret_data) => {
                let balance = U256::from_be_slice(&ret_data[0..32]);
                Ok(balance)
            }
            Err(e) => {
                trace!("balances256 call error {} : {}", coin_id, e);
                if coin_id == 0 {
                    Self::balance128(client, address, coin_id).await
                } else {
                    Err(eyre!("CANNOT_GET_COIN_BALANCE"))
                }
            }
        }
    }
    pub async fn coins(client: P, address: Address) -> Result<Vec<Address>> {
        let mut ret: Vec<Address> = Vec::new();
        for i in 0..4 {
            match Self::coin(client.clone(), address, i).await {
                Ok(coin_address) => ret.push(coin_address),
                Err(_) => break,
            }
        }
        if ret.is_empty() {
            trace!("coin fetch coins");
            Err(eyre!("CANNOT_GET_COIN_ADDRESS"))
        } else {
            trace!("coins @{} {:?}", address, ret);
            Ok(ret)
        }
    }

    pub async fn balances(client: P, address: Address) -> Result<Vec<U256>> {
        let mut ret: Vec<U256> = Vec::new();

        let common_contract = ICurveCommonInstance::new(address, client.clone());
        match common_contract.get_balances().call().await {
            Ok(return_bytes) => {
                if return_bytes._0.len() < 64 {
                    return Err(eyre!("CANNOT_FETCH_BALANCES"));
                }
                let balances_count = U256::from_be_slice(&return_bytes._0.to_vec()[0..32]);
                for i in 0usize..balances_count.to() {
                    let balance = U256::from_be_slice(&return_bytes._0.to_vec()[32 + i * 32..64 + i * 32]);
                    ret.push(balance)
                }
                debug!("Curve Balances {:?} {:?}", address, ret);
                Ok(ret)
            }
            Err(e) => {
                for i in 0..4 {
                    match Self::balance(client.clone(), address, i).await {
                        Ok(balance) => ret.push(balance),
                        Err(e) => {
                            trace!("Error fetching coin balance {} : {}", i, e);
                            break;
                        }
                    }
                }
                if ret.is_empty() {
                    trace!("coin call error {}", e);
                    Err(eyre!("CANNOT_GET_COIN_BALANCE"))
                } else {
                    Ok(ret)
                }
            }
        }
    }
}

impl<P, N> CurveContract<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn get_address(&self) -> Address {
        match self {
            CurveContract::I128_2(interface) => *interface.address(),

            CurveContract::I128_2ToMeta(interface) => *interface.address(),

            CurveContract::I128_2To(interface) => *interface.address(),
            CurveContract::I128_3(interface) => *interface.address(),
            CurveContract::I128_4(interface) => *interface.address(),
            CurveContract::U256_2(interface) => *interface.address(),
            CurveContract::U256_2To(interface) => *interface.address(),
            CurveContract::U256_2EthTo(interface) => *interface.address(),
            CurveContract::U256_3Eth(interface) => *interface.address(),
            CurveContract::U256_3EthTo(interface) => *interface.address(),
            CurveContract::U256_3EthTo2(interface) => *interface.address(),
        }
    }

    pub fn can_exchange_to(&self) -> bool {
        !(matches!(self, CurveContract::I128_3(_)) | matches!(self, CurveContract::U256_3Eth(_)))
    }

    pub fn can_calculate_in_amount(&self) -> bool {
        matches!(
            self,
            CurveContract::I128_2To(_) | CurveContract::U256_2EthTo(_) | CurveContract::U256_3EthTo(_) | CurveContract::I128_2ToMeta(_)
        )
    }

    pub async fn get_dy(&self, i: u32, j: u32, amount: U256) -> Result<U256> {
        match self {
            CurveContract::I128_2(interface) => match interface.get_dy(i.into(), j.into(), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::I128_2ToMeta(interface) => match interface.get_dy(i.into(), j.into(), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::I128_2To(interface) => match interface.get_dy(i.into(), j.into(), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::I128_3(interface) => match interface.get_dy(i.into(), j.into(), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::I128_4(interface) => match interface.get_dy(i.into(), j.into(), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::U256_2(interface) => match interface.get_dy(U256::from(i), U256::from(j), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::U256_2To(interface) => match interface.get_dy(U256::from(i), U256::from(j), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::U256_2EthTo(interface) => match interface.get_dy(U256::from(i), U256::from(j), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::U256_3EthTo(interface) => match interface.get_dy(U256::from(i), U256::from(j), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::U256_3EthTo2(interface) => match interface.get_dy(U256::from(i), U256::from(j), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
            CurveContract::U256_3Eth(interface) => match interface.get_dy(U256::from(i), U256::from(j), amount).call().await {
                Ok(x) => Ok(x._0),
                _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR")),
            },
        }
    }

    pub fn get_dx_call_data(&self, i: u32, j: u32, amount: U256) -> Result<Bytes> {
        let ret: Result<Bytes, Report> = match self {
            CurveContract::I128_2To(interface) => Ok(interface.get_dx(i.into(), j.into(), amount).calldata().clone()),
            CurveContract::U256_3EthTo(interface) => Ok(interface.get_dx(U256::from(i), U256::from(j), amount).calldata().clone()),
            CurveContract::U256_3EthTo2(interface) => Ok(interface.get_dx(U256::from(i), U256::from(j), amount).calldata().clone()),
            _ => Err(eyre!("CURVE_CANNOT_CALC_DX")),
        };
        ret
    }

    pub fn get_dy_underlying_call_data(&self, i: u32, j: u32, amount: U256) -> Result<Bytes> {
        match self {
            CurveContract::I128_2ToMeta(interface) => Ok(interface.get_dy_underlying(i.into(), j.into(), amount).calldata().clone()),
            _ => Err(eyre!("GET_DY_UNDERLYING_NOT_SUPPORTED")),
        }
    }

    pub fn get_dy_call_data(&self, i: u32, j: u32, amount: U256) -> Result<Bytes> {
        match self {
            CurveContract::I128_2(interface) => Ok(interface.get_dy(i.into(), j.into(), amount).calldata().clone()),
            CurveContract::I128_2ToMeta(interface) => Ok(interface.get_dy(i.into(), j.into(), amount).calldata().clone()),

            CurveContract::I128_2To(interface) => Ok(interface.get_dy(i.into(), j.into(), amount).calldata().clone()),
            CurveContract::I128_3(interface) => Ok(interface.get_dy(i.into(), j.into(), amount).calldata().clone()),
            CurveContract::I128_4(interface) => Ok(interface.get_dy(i.into(), j.into(), amount).calldata().clone()),
            CurveContract::U256_2(interface) => Ok(interface.get_dy(U256::from(i), U256::from(j), amount).calldata().clone()),
            CurveContract::U256_2To(interface) => Ok(interface.get_dy(U256::from(i), U256::from(j), amount).calldata().clone()),
            CurveContract::U256_2EthTo(interface) => Ok(interface.get_dy(U256::from(i), U256::from(j), amount).calldata().clone()),
            CurveContract::U256_3Eth(interface) => Ok(interface.get_dy(U256::from(i), U256::from(j), amount).calldata().clone()),
            CurveContract::U256_3EthTo(interface) => Ok(interface.get_dy(U256::from(i), U256::from(j), amount).calldata().clone()),
            CurveContract::U256_3EthTo2(interface) => Ok(interface.get_dy(U256::from(i), U256::from(j), amount).calldata().clone()),
        }
    }

    pub fn calc_token_amount_call_data(&self, i: u32, amount: U256) -> Result<Bytes> {
        match self {
            CurveContract::I128_2(interface) => {
                let mut amounts: [U256; 2] = Default::default();
                amounts[i as usize] = amount;
                Ok(interface.calc_token_amount(amounts, true).calldata().clone())
            }
            CurveContract::I128_3(interface) => {
                let mut amounts: [U256; 3] = Default::default();
                amounts[i as usize] = amount;
                Ok(interface.calc_token_amount(amounts, true).calldata().clone())
            }
            CurveContract::I128_4(interface) => {
                let mut amounts: [U256; 4] = Default::default();
                amounts[i as usize] = amount;
                Ok(interface.calc_token_amount(amounts, true).calldata().clone())
            }
            _ => Err(eyre!("CURVE_TOKEN_AMOUNT_CALL_DATA_NOT_SUPPORTED")),
        }
    }

    pub fn calc_withdraw_one_coin_call_data(&self, i: u32, amount: U256) -> Result<Bytes> {
        match self {
            CurveContract::I128_3(interface) => Ok(interface.calc_withdraw_one_coin(amount, i.into()).calldata().clone()),
            _ => Err(eyre!("CURVE_WITHDRAW_ONE_COIN_NOT_SUPPORTED")),
        }
    }

    pub fn get_exchange_underlying_call_data(&self, i: u32, j: u32, amount: U256, min_dy: U256, to: Address) -> Result<Bytes> {
        match self {
            CurveContract::I128_2ToMeta(interface) => {
                Ok(interface.exchange_underlying(i.into(), j.into(), amount, min_dy, to).calldata().clone())
            }
            _ => Err(eyre!("GET_EXCHANGE_UNDERLYING_CALL_DATA_NOT_SUPPORTED")),
        }
    }

    pub fn get_exchange_call_data(&self, i: u32, j: u32, amount: U256, min_dy: U256, to: Address) -> Result<Bytes> {
        match self {
            CurveContract::I128_2(interface) => Ok(interface.exchange(i.into(), j.into(), amount, min_dy).calldata().clone()),
            CurveContract::I128_2ToMeta(interface) => Ok(interface.exchange(i.into(), j.into(), amount, min_dy, to).calldata().clone()),
            CurveContract::I128_2To(interface) => Ok(interface.exchange(i.into(), j.into(), amount, min_dy, to).calldata().clone()),
            CurveContract::I128_3(interface) => Ok(interface.exchange(i.into(), j.into(), amount, min_dy).calldata().clone()),
            CurveContract::I128_4(interface) => Ok(interface.exchange(i.into(), j.into(), amount, min_dy).calldata().clone()),
            CurveContract::U256_2(interface) => Ok(interface.exchange(U256::from(i), U256::from(j), amount, min_dy).calldata().clone()),
            CurveContract::U256_2To(interface) => {
                Ok(interface.exchange(U256::from(i), U256::from(j), amount, min_dy, to).calldata().clone())
            }
            CurveContract::U256_2EthTo(interface) => {
                Ok(interface.exchange(U256::from(i), U256::from(j), amount, min_dy, false, to).calldata().clone())
            }
            CurveContract::U256_3Eth(interface) => {
                Ok(interface.exchange(U256::from(i), U256::from(j), amount, min_dy, false).calldata().clone())
            }
            CurveContract::U256_3EthTo(interface) => {
                Ok(interface.exchange(U256::from(i), U256::from(j), amount, min_dy, false, to).calldata().clone())
            }
            CurveContract::U256_3EthTo2(interface) => {
                Ok(interface.exchange(U256::from(i), U256::from(j), amount, min_dy, false, to).calldata().clone())
            }
        }
    }

    pub fn get_add_liquidity_call_data(&self, i: u32, amount: U256, _to: Address) -> Result<Bytes> {
        match self {
            CurveContract::I128_3(interface) => {
                let mut amounts: [U256; 3] = Default::default();
                amounts[i as usize] = amount;
                Ok(interface.add_liquidity(amounts, U256::ZERO).calldata().clone())
            }
            _ => Err(eyre!("ADD_LIQUIDITY_NOT_SUPPORTED")),
        }
    }

    pub fn get_remove_liquidity_one_coin_call_data(&self, i: u32, amount: U256, _to: Address) -> Result<Bytes> {
        match self {
            CurveContract::I128_3(interface) => Ok(interface.remove_liquidity_one_coin(amount, i as i128, U256::ZERO).calldata().clone()),
            _ => Err(eyre!("REMOVE_LIQUIDITY_ONE_COIN_NOT_SUPPORTED")),
        }
    }
}

pub struct CurveProtocol<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    p: PhantomData<(P, N)>,
}

impl<P, N> CurveProtocol<P, N>
where
    N: Network,
    P: Provider<N> + Send + Sync + Clone + 'static,
{
    pub fn get_underlying_tokens(meta_token_address: Address) -> Result<Vec<Address>> {
        if meta_token_address == address!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490") {
            Ok(vec![
                address!("6B175474E89094C44Da98b954EedeAC495271d0F"),
                address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
            ])
        } else if meta_token_address == address!("3175Df0976dFA876431C2E9eE6Bc45b65d3473CC") {
            Ok(vec![address!("853d955aCEf822Db058eb8505911ED77F175b99e"), address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")])
        } else {
            Err(eyre!("META_POOL_NOT_FOUND"))
        }
    }

    pub fn new_i128_2(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveI128_2Instance::new(address, client);
        CurveContract::I128_2(contract)
    }

    pub fn new_i128_2_to_meta(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveI128_2_To_MetaInstance::new(address, client);
        CurveContract::I128_2ToMeta(contract)
    }

    pub fn new_i128_2_to(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveI128_2_To::new(address, client);
        CurveContract::I128_2To(contract)
    }
    pub fn new_i128_3(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveI128_3::new(address, client);
        CurveContract::I128_3(contract)
    }
    pub fn new_i128_4(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveI128_4::new(address, client);
        CurveContract::I128_4(contract)
    }
    pub fn new_u256_2(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveU256_2::new(address, client);
        CurveContract::U256_2(contract)
    }

    pub fn new_u256_2_to(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveU256_2_To::new(address, client);
        CurveContract::U256_2To(contract)
    }

    pub fn new_u256_2_eth_to(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveU256_2_Eth_To::new(address, client);
        CurveContract::U256_2EthTo(contract)
    }

    pub fn new_u256_3_eth_to(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveU256_3_Eth_To::new(address, client);
        CurveContract::U256_3EthTo(contract)
    }
    pub fn new_u256_3_eth_to2(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveU256_3_Eth_To2::new(address, client);
        CurveContract::U256_3EthTo2(contract)
    }

    pub fn new_u256_3_eth(client: P, address: Address) -> CurveContract<P, N> {
        let contract = ICurveU256_3_Eth::new(address, client);
        CurveContract::U256_3Eth(contract)
    }

    /*
    I128_3
    0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7 // DAI-USDT-USDC

    I128_2_to
    0x4DEcE678ceceb27446b35C672dC7d61F30bAD69E // crvUSD-USDC

    U256_2_Eth_to
    0x9409280DC1e6D33AB7A8C6EC03e5763FB61772B5  // LDO-ETH

    U256_3_Eth
    0xD51a44d3FaE010294C616388b506AcdA1bfAAE46 // WETH-WBTC-USDT


    U256_3_Eth_to
    0x7F86Bf177Dd4F3494b841a37e810A34dD56c829B // WETH-WBTC-USDC
    0xf5f5B97624542D72A9E06f04804Bf81baA15e2B4 // WETH-WBTC-USDT
     */

    fn match_abi(code: &Bytes, abi: Vec<[u8; 4]>) -> bool {
        //println!("Code len {}", code.len());
        for f in abi.iter() {
            if !code.as_ref().windows(4).any(|sig| sig == f) {
                //println!("{} not found", fn_name);
                return false;
            } else {
                //println!("{} found", fn_name);
            }
        }

        true
    }

    pub async fn get_factory_address(client: P, id: u32) -> Result<Address> {
        let address_provider_address: Address = "0x0000000022D53366457F9d5E68Ec105046FC4383".parse().unwrap();
        let address_provider = ICurveAddressProviderInstance::new(address_provider_address, client);
        match address_provider.get_address(U256::from(id)).call().await {
            Ok(x) => Ok(x._0),
            Err(e) => {
                error!("Error getting factory address : {}", e);
                Err(eyre!("GET_FACTORY_ADDRESS_ERROR"))
            }
        }
    }

    pub async fn get_pool_address(client: P, factory_address: Address, pool_id: u32) -> Result<Address> {
        let factory = ICurveFactoryInstance::new(factory_address, client);
        match factory.pool_list(U256::from(pool_id)).call().await {
            Ok(x) => Ok(x._0),
            Err(e) => {
                error!("Error getting factory address :{}", e);
                Err(eyre!("GET_POOL_ADDRESS_ERROR"))
            }
        }
    }

    pub async fn get_pool_count(client: P, factory_address: Address) -> Result<u32> {
        let factory = ICurveFactoryInstance::new(factory_address, client);
        match factory.pool_count().call().await {
            Ok(x) => Ok(x._0.to()),
            Err(e) => {
                error!("Error getting pool count : {}", e);
                Err(eyre!("GET_POOL_COUNT_ERROR"))
            }
        }
    }

    pub async fn get_contract_from_code(client: P, address: Address) -> Result<CurveContract<P, N>> {
        //let sig = ICurveU256_3_EthCalls::Balances(  <ICurveU256_3_Eth<M>>::BalancesCall );
        //let sig = ICurveU256_3_EthCalls::Balances(  BalancesCall{} );

        let mut code = client.get_code_at(address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await?;

        if code.len() < 100 {
            for i in 20..code.len() - 1 {
                if code[i] == 0x5A && code[i + 1] == 0xF4 {
                    let underlying_address = Address::from_slice(&code.to_vec()[i - 20..i]);
                    debug!("get_contract_from_code Underlying address {}", underlying_address);
                    code = client.get_code_at(underlying_address).block_id(BlockId::Number(BlockNumberOrTag::Latest)).await?;
                    break;
                }
            }
        }

        let code: Bytes = code;

        if code.len() < 100 {
            return Err(eyre!("CANNOT_FIND_UNDERLYING"));
        }

        if Self::match_abi(&code, ICurveI128_2_To_Meta::ICurveI128_2_To_MetaCalls::selectors().collect()) {
            return Ok(Self::new_i128_2_to_meta(client, address));
        }

        if Self::match_abi(&code, ICurveI128_2_ToCalls::selectors().collect()) {
            return Ok(Self::new_i128_2_to(client, address));
        }

        if Self::match_abi(&code, ICurveI128_2::ICurveI128_2Calls::selectors().collect()) {
            return Ok(Self::new_i128_2(client, address));
        }
        if Self::match_abi(&code, ICurveI128_3Calls::selectors().collect()) {
            return Ok(Self::new_i128_3(client, address));
        }
        if Self::match_abi(&code, ICurveI128_4Calls::selectors().collect()) {
            return Ok(Self::new_i128_4(client, address));
        }
        if Self::match_abi(&code, ICurveU256_2_ToCalls::selectors().collect()) {
            return Ok(Self::new_u256_2_to(client, address));
        }
        if Self::match_abi(&code, ICurveU256_2Calls::selectors().collect()) {
            return Ok(Self::new_u256_2(client, address));
        }
        if Self::match_abi(&code, ICurveU256_2_Eth_ToCalls::selectors().collect()) {
            return Ok(Self::new_u256_2_eth_to(client, address));
        }
        if Self::match_abi(&code, ICurveU256_3_EthCalls::selectors().collect()) {
            return Ok(Self::new_u256_3_eth(client, address));
        }
        if Self::match_abi(&code, ICurveU256_3_Eth_ToCalls::selectors().collect()) {
            return Ok(Self::new_u256_3_eth_to(client, address));
        }
        if Self::match_abi(&code, ICurveU256_3_Eth_To2Calls::selectors().collect()) {
            return Ok(Self::new_u256_3_eth_to2(client, address));
        }

        Err(eyre!("ABI_NOT_FOUND"))
    }

    pub fn get_contracts_vec(client: P) -> Vec<CurveContract<P, N>> {
        vec![
            Self::new_u256_3_eth_to(client.clone(), address!("f5f5B97624542D72A9E06f04804Bf81baA15e2B4")),
            //Self::new_u256_3_eth_to(client.clone(), "0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490".parse().unwrap()),
            Self::new_i128_3(client.clone(), address!("bEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7")),
            Self::new_i128_2_to(client.clone(), address!("4DEcE678ceceb27446b35C672dC7d61F30bAD69E")),
            Self::new_u256_2_eth_to(client.clone(), address!("9409280DC1e6D33AB7A8C6EC03e5763FB61772B5")),
            Self::new_u256_3_eth(client.clone(), address!("D51a44d3FaE010294C616388b506AcdA1bfAAE46")),
            Self::new_u256_3_eth_to(client.clone(), address!("7F86Bf177Dd4F3494b841a37e810A34dD56c829B")),
            Self::new_i128_2(client.clone(), address!("DC24316b9AE028F1497c275EB9192a3Ea0f67022")),
            Self::new_i128_2_to(client.clone(), address!("828b154032950C8ff7CF8085D841723Db2696056")),
            //Self::new_i128_2_to_meta(client.clone(), address!("Ed279fDD11cA84bEef15AF5D39BB4d4bEE23F0cA".parse().unwrap()),
        ]
    }
}
