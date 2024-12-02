use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use crate::required_state::RequiredState;
use alloy_primitives::{Address, Bytes, B256, U256};
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
    #[serde(rename = "maverick")]
    #[strum(serialize = "maverick")]
    Maverick,
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
    #[serde(rename = "custom")]
    #[strum(serialize = "custom")]
    Custom(u64),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PoolProtocol {
    Unknown,
    UniswapV2,
    UniswapV2Like,
    NomiswapStable,
    Sushiswap,
    SushiswapV3,
    DooarSwap,
    OgPepe,
    Safeswap,
    Miniswap,
    Shibaswap,
    UniswapV3,
    UniswapV3Like,
    PancakeV3,
    Integral,
    Maverick,
    Curve,
    LidoStEth,
    LidoWstEth,
    RocketEth,
    Custom(u64),
}

impl Display for PoolProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let protocol_name = match self {
            Self::Unknown => "Unknown",
            Self::UniswapV2 => "UniswapV2",
            Self::UniswapV2Like => "UniswapV2Like",
            Self::UniswapV3 => "UniswapV3",
            Self::PancakeV3 => "PancakeV3",
            Self::UniswapV3Like => "UniswapV3Like",
            Self::NomiswapStable => "NomiswapStable",
            Self::Sushiswap => "Sushiswap",
            Self::SushiswapV3 => "SushiswapV3",
            Self::DooarSwap => "Dooarswap",
            Self::OgPepe => "OgPepe",
            Self::Miniswap => "Miniswap",
            Self::Shibaswap => "Shibaswap",
            Self::Safeswap => "Safeswap",
            Self::Integral => "Integral",
            Self::Maverick => "Maverick",
            Self::Curve => "Curve",
            Self::LidoWstEth => "WstEth",
            Self::LidoStEth => "StEth",
            Self::RocketEth => "RocketEth",
            Self::Custom(x) => "Custom",
        };
        write!(f, "{}", protocol_name)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PoolId<LDT: LoomDataTypes = LoomDataTypesEthereum>
where
    LDT::Address: Eq + Hash,
{
    Address(LDT::Address),
    Bytes32(B256),
}

impl<LDT: LoomDataTypes> PoolId<LDT> {
    pub fn address_or_zero(&self) -> LDT::Address {
        if let Self::Address(addr) = self {
            *addr
        } else {
            LDT::Address::default()
        }
    }

    pub fn bytes_or_zero(&self) -> B256 {
        if let Self::Bytes32(addr) = self {
            *addr
        } else {
            B256::ZERO
        }
    }
}

impl<LDT: LoomDataTypes> Copy for PoolId<LDT> {}

impl<LDT: LoomDataTypes> Hash for PoolId<LDT> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Address(addr) => addr.hash(state),
            Self::Bytes32(addr) => addr.hash(state),
        }
    }
}

impl<LDT: LoomDataTypes> PartialEq<Self> for PoolId<LDT> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Address(a), Self::Address(b)) => a == b,
            (Self::Bytes32(a), Self::Bytes32(b)) => a == b,
            _ => false,
        }
    }
}

impl<LDT: LoomDataTypes> PartialOrd for PoolId<LDT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<LDT: LoomDataTypes> Ord for PoolId<LDT> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (PoolId::Address(a), PoolId::Address(b)) => a.cmp(b),
            (PoolId::Bytes32(a), PoolId::Bytes32(b)) => a.cmp(b),
            (PoolId::Address(a), PoolId::Bytes32(b)) => Ordering::Less,
            (PoolId::Bytes32(a), PoolId::Address(b)) => Ordering::Greater,
        }
    }
}

impl<LDT: LoomDataTypes> Display for PoolId<LDT> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Address(a) => write!(f, "{}", a),
            Self::Bytes32(a) => write!(f, "{}", a),
        }
    }
}

impl<LDT: LoomDataTypes> Eq for PoolId<LDT> {}

impl<LDT: LoomDataTypes> Default for PoolId<LDT> {
    fn default() -> Self {
        Self::Address(Default::default())
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
    fn get_class(&self) -> PoolClass {
        PoolClass::Unknown
    }

    fn get_protocol(&self) -> PoolProtocol {
        PoolProtocol::Unknown
    }

    fn get_address(&self) -> LDT::Address;

    fn get_pool_id(&self) -> PoolId<LDT>;

    fn get_fee(&self) -> U256 {
        U256::ZERO
    }

    fn get_tokens(&self) -> Vec<LDT::Address> {
        Vec::new()
    }

    fn get_swap_directions(&self) -> Vec<(LDT::Address, LDT::Address)> {
        Vec::new()
    }

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

    fn can_calculate_in_amount(&self) -> bool {
        true
    }

    fn get_encoder(&self) -> Option<&dyn PoolAbiEncoder>;

    fn get_read_only_cell_vec(&self) -> Vec<U256> {
        Vec::new()
    }

    fn get_state_required(&self) -> Result<RequiredState>;

    fn is_native(&self) -> bool;
}

pub struct DefaultAbiSwapEncoder {}

impl PoolAbiEncoder for DefaultAbiSwapEncoder {}

#[derive(Clone, Debug, PartialEq)]
pub enum PreswapRequirement {
    Unknown,
    Transfer(Address),
    Allowance,
    Callback,
    Base,
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
    fn preswap_requirement(&self) -> PreswapRequirement {
        PreswapRequirement::Unknown
    }

    fn is_native(&self) -> bool {
        false
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
