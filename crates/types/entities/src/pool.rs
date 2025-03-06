use std::any::Any;
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use crate::required_state::RequiredState;
use crate::swap_direction::SwapDirection;
use crate::PoolId;
use alloy_primitives::{Address, Bytes, U256};
use eyre::{eyre, ErrReport, Result};
use loom_defi_address_book::FactoryAddress;
use loom_types_blockchain::{LoomDataTypes, LoomDataTypesEthereum};
use revm::primitives::Env;
use revm::DatabaseRef;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString, VariantNames};

pub fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
    if factory_address == FactoryAddress::UNISWAP_V2 {
        PoolProtocol::UniswapV2
    } else if factory_address == FactoryAddress::UNISWAP_V3 {
        PoolProtocol::UniswapV3
    } else if factory_address == FactoryAddress::PANCAKE_V3 {
        PoolProtocol::PancakeV3
    } else if factory_address == FactoryAddress::NOMISWAP {
        PoolProtocol::NomiswapStable
    } else if factory_address == FactoryAddress::ANTFARM {
        PoolProtocol::AntFarm
    } else if factory_address == FactoryAddress::SUSHISWAP_V2 {
        PoolProtocol::Sushiswap
    } else if factory_address == FactoryAddress::SUSHISWAP_V3 {
        PoolProtocol::SushiswapV3
    } else if factory_address == FactoryAddress::DOOARSWAP {
        PoolProtocol::DooarSwap
    } else if factory_address == FactoryAddress::SAFESWAP {
        PoolProtocol::Safeswap
    } else if factory_address == FactoryAddress::MINISWAP {
        PoolProtocol::Miniswap
    } else if factory_address == FactoryAddress::SHIBASWAP {
        PoolProtocol::Shibaswap
    } else if factory_address == FactoryAddress::MAVERICK {
        PoolProtocol::Maverick
    } else if factory_address == FactoryAddress::INTEGRAL {
        PoolProtocol::Integral
    } else {
        PoolProtocol::Unknown
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq, EnumString, VariantNames, Display, Default, Deserialize, Serialize, EnumIter)]
#[strum(ascii_case_insensitive, serialize_all = "lowercase")]
pub enum PoolClass {
    #[default]
    #[serde(rename = "unknown")]
    #[strum(serialize = "unknown")]
    Unknown,
    #[serde(rename = "uniswap2")]
    #[strum(serialize = "uniswap2")]
    UniswapV2,
    #[serde(rename = "uniswap3")]
    #[strum(serialize = "uniswap3")]
    UniswapV3,
    #[serde(rename = "uniswap4")]
    #[strum(serialize = "uniswap4")]
    UniswapV4,
    #[serde(rename = "maverick")]
    #[strum(serialize = "maverick")]
    Maverick,
    #[serde(rename = "maverick2")]
    #[strum(serialize = "maverick2")]
    MaverickV2,
    #[serde(rename = "pancake3")]
    #[strum(serialize = "pancake3")]
    PancakeV3,
    #[serde(rename = "curve")]
    #[strum(serialize = "curve")]
    Curve,
    #[serde(rename = "steth")]
    #[strum(serialize = "steth")]
    LidoStEth,
    #[serde(rename = "wsteth")]
    #[strum(serialize = "wsteth")]
    LidoWstEth,
    #[serde(rename = "rocketpool")]
    #[strum(serialize = "rocketpool")]
    RocketPool,
    #[serde(rename = "balancer1")]
    #[strum(serialize = "balancer1")]
    BalancerV1,
    #[serde(rename = "balancer2")]
    #[strum(serialize = "balancer2")]
    BalancerV2,
    #[serde(rename = "custom")]
    #[strum(serialize = "custom")]
    Custom(u64),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolProtocol {
    Unknown,
    AaveV2,
    AaveV3,
    UniswapV2,
    UniswapV2Like,
    NomiswapStable,
    Sushiswap,
    SushiswapV3,
    DooarSwap,
    OgPepe,
    AntFarm,
    Safeswap,
    Miniswap,
    Shibaswap,
    UniswapV3,
    UniswapV3Like,
    UniswapV4,
    PancakeV3,
    Integral,
    Maverick,
    MaverickV2,
    Curve,
    LidoStEth,
    LidoWstEth,
    RocketEth,
    BalancerV1,
    BalancerV2,
    Custom(u64),
}

impl Display for PoolProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let protocol_name = match self {
            Self::Unknown => "Unknown",
            Self::AaveV2 => "AaveV2",
            Self::AaveV3 => "AaveV3",
            Self::UniswapV2 => "UniswapV2",
            Self::UniswapV2Like => "UniswapV2Like",
            Self::UniswapV3 => "UniswapV3",
            Self::PancakeV3 => "PancakeV3",
            Self::UniswapV4 => "UniswapV4",
            Self::UniswapV3Like => "UniswapV3Like",
            Self::NomiswapStable => "NomiswapStable",
            Self::Sushiswap => "Sushiswap",
            Self::SushiswapV3 => "SushiswapV3",
            Self::DooarSwap => "Dooarswap",
            Self::OgPepe => "OgPepe",
            Self::AntFarm => "AntFarm",
            Self::Miniswap => "Miniswap",
            Self::Shibaswap => "Shibaswap",
            Self::Safeswap => "Safeswap",
            Self::Integral => "Integral",
            Self::Maverick => "Maverick",
            Self::MaverickV2 => "MaverickV2",
            Self::Curve => "Curve",
            Self::LidoWstEth => "WstEth",
            Self::LidoStEth => "StEth",
            Self::RocketEth => "RocketEth",
            Self::BalancerV1 => "BalancerV1",
            Self::BalancerV2 => "BalancerV2",
            Self::Custom(x) => "Custom",
        };
        write!(f, "{}", protocol_name)
    }
}

pub struct PoolWrapper<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    pub pool: Arc<dyn Pool<LDT>>,
}

impl<LDT: LoomDataTypes> PartialOrd for PoolWrapper<LDT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<LDT: LoomDataTypes> Eq for PoolWrapper<LDT> {}

impl<LDT: LoomDataTypes> Ord for PoolWrapper<LDT> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get_address().cmp(&other.get_address())
    }
}

impl<LDT: LoomDataTypes> Display for PoolWrapper<LDT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.get_protocol(), self.get_address())
    }
}

impl<LDT: LoomDataTypes> Debug for PoolWrapper<LDT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.get_protocol(), self.get_address())
    }
}

impl<LDT: LoomDataTypes> Hash for PoolWrapper<LDT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get_address().hash(state)
    }
}

impl<LDT: LoomDataTypes> PartialEq for PoolWrapper<LDT> {
    fn eq(&self, other: &Self) -> bool {
        self.pool.get_address() == other.pool.get_address()
    }
}

impl<LDT: LoomDataTypes> Clone for PoolWrapper<LDT> {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

impl<LDT: LoomDataTypes> Deref for PoolWrapper<LDT> {
    type Target = dyn Pool<LDT>;

    fn deref(&self) -> &Self::Target {
        self.pool.deref()
    }
}

impl<LDT: LoomDataTypes> AsRef<dyn Pool<LDT>> for PoolWrapper<LDT> {
    fn as_ref(&self) -> &(dyn Pool<LDT> + 'static) {
        self.pool.as_ref()
    }
}

impl<LDT: LoomDataTypes> PoolWrapper<LDT> {
    pub fn new(pool: Arc<dyn Pool<LDT>>) -> Self {
        PoolWrapper { pool }
    }
}

impl<T: 'static + Pool<LoomDataTypesEthereum>> From<T> for PoolWrapper<LoomDataTypesEthereum> {
    fn from(pool: T) -> Self {
        Self { pool: Arc::new(pool) }
    }
}

pub trait Pool<LDT: LoomDataTypes = LoomDataTypesEthereum>: Sync + Send {
    fn as_any(&self) -> &dyn Any;

    fn get_class(&self) -> PoolClass;

    fn get_protocol(&self) -> PoolProtocol;

    fn get_address(&self) -> LDT::Address;

    fn get_pool_id(&self) -> PoolId<LDT>;

    fn get_fee(&self) -> U256;

    fn get_tokens(&self) -> Vec<LDT::Address>;

    fn get_swap_directions(&self) -> Vec<SwapDirection<LDT>>;

    fn calculate_out_amount(
        &self,
        state: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &LDT::Address,
        token_address_to: &LDT::Address,
        in_amount: U256,
    ) -> Result<(U256, u64), ErrReport>;

    // returns (in_amount, gas_used)
    fn calculate_in_amount(
        &self,
        state: &dyn DatabaseRef<Error = ErrReport>,
        env: Env,
        token_address_from: &LDT::Address,
        token_address_to: &LDT::Address,
        out_amount: U256,
    ) -> Result<(U256, u64), ErrReport>;

    fn can_flash_swap(&self) -> bool;

    fn can_calculate_in_amount(&self) -> bool;

    fn get_abi_encoder(&self) -> Option<&dyn PoolAbiEncoder>;

    fn get_read_only_cell_vec(&self) -> Vec<U256>;

    fn get_state_required(&self) -> Result<RequiredState>;

    fn is_native(&self) -> bool;

    fn preswap_requirement(&self) -> PreswapRequirement;

    fn get_pool_manager_cells(&self) -> Vec<(Address, Vec<U256>)> {
        vec![]
    }
}

pub struct DefaultAbiSwapEncoder {}

impl PoolAbiEncoder for DefaultAbiSwapEncoder {}

#[derive(Clone, Debug)]
pub enum PreswapRequirement<LDT: LoomDataTypes = LoomDataTypesEthereum> {
    Unknown,
    Transfer(LDT::Address),
    Allowance,
    Callback,
    Base,
}

impl<LDT> PartialEq for PreswapRequirement<LDT>
where
    LDT: LoomDataTypes,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PreswapRequirement::Unknown, PreswapRequirement::Unknown) => true,
            (PreswapRequirement::Transfer(addr1), PreswapRequirement::Transfer(addr2)) => addr1 == addr2,
            (PreswapRequirement::Allowance, PreswapRequirement::Allowance) => true,
            (PreswapRequirement::Callback, PreswapRequirement::Callback) => true,
            (PreswapRequirement::Base, PreswapRequirement::Base) => true,
            _ => false,
        }
    }
}

impl<LDT: LoomDataTypes> PreswapRequirement<LDT> {
    pub fn address_or(&self, default_address: LDT::Address) -> LDT::Address {
        match self {
            PreswapRequirement::Transfer(address) => *address,
            _ => default_address,
        }
    }
    pub fn address(&self) -> Option<LDT::Address> {
        match self {
            PreswapRequirement::Transfer(address) => Some(*address),
            _ => None,
        }
    }
}

pub trait PoolAbiEncoder: Send + Sync {
    fn encode_swap_in_amount_provided(
        &self,
        _token_from_address: Address,
        _token_to_address: Address,
        _amount: U256,
        _recipient: Address,
        _payload: Bytes,
    ) -> Result<Bytes> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }
    fn encode_swap_out_amount_provided(
        &self,
        _token_from_address: Address,
        _token_to_address: Address,
        _amount: U256,
        _recipient: Address,
        _payload: Bytes,
    ) -> Result<Bytes> {
        Err(eyre!("NOT_IMPLEMENTED"))
    }

    fn swap_in_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_out_amount_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_out_amount_return_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_in_amount_return_offset(&self, _token_from_address: Address, _token_to_address: Address) -> Option<u32> {
        None
    }
    fn swap_out_amount_return_script(&self, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }
    fn swap_in_amount_return_script(&self, _token_from_address: Address, _token_to_address: Address) -> Option<Bytes> {
        None
    }
}

#[cfg(test)]
mod test {
    use crate::PoolClass;

    #[test]
    fn test_strum() {
        println!("{}", PoolClass::Unknown);
        println!("{}", PoolClass::UniswapV2);
    }
}
