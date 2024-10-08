use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use crate::required_state::RequiredState;
use alloy_primitives::{address, Address, Bytes, U256};
use eyre::{eyre, ErrReport, Result};
use loom_revm_db::LoomInMemoryDB;
use revm::primitives::Env;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString, VariantNames};

const UNISWAPV2_FACTORY: Address = address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f");
const NOMISWAP_STABLE_FACTORY: Address = address!("818339b4E536E707f14980219037c5046b049dD4");
const SUSHISWAP_FACTORY: Address = address!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac");
const DOOARSWAP_FACTORY: Address = address!("1e895bFe59E3A5103e8B7dA3897d1F2391476f3c");
const SAFESWAP_FACTORY: Address = address!("7F09d4bE6bbF4b0fF0C97ca5c486a166198aEAeE");
const UNISWAPV3_FACTORY: Address = address!("1F98431c8aD98523631AE4a59f267346ea31F984");
const PANCAKEV3_FACTORY: Address = address!("0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865");
const MINISWAP_FACTORY: Address = address!("2294577031F113DF4782B881cF0b140e94209a6F");
const SHIBASWAP_FACTORY: Address = address!("115934131916C8b277DD010Ee02de363c09d037c");
const MAVERICK_FACTORY: Address = address!("Eb6625D65a0553c9dBc64449e56abFe519bd9c9B");

pub fn get_protocol_by_factory(factory_address: Address) -> PoolProtocol {
    if factory_address == UNISWAPV2_FACTORY {
        PoolProtocol::UniswapV2
    } else if factory_address == UNISWAPV3_FACTORY {
        PoolProtocol::UniswapV3
    } else if factory_address == PANCAKEV3_FACTORY {
        PoolProtocol::PancakeV3
    } else if factory_address == NOMISWAP_STABLE_FACTORY {
        PoolProtocol::NomiswapStable
    } else if factory_address == SUSHISWAP_FACTORY {
        PoolProtocol::Sushiswap
    } else if factory_address == DOOARSWAP_FACTORY {
        PoolProtocol::DooarSwap
    } else if factory_address == SAFESWAP_FACTORY {
        PoolProtocol::Safeswap
    } else if factory_address == MINISWAP_FACTORY {
        PoolProtocol::Miniswap
    } else if factory_address == SHIBASWAP_FACTORY {
        PoolProtocol::Shibaswap
    } else if factory_address == MAVERICK_FACTORY {
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
    //NomiswapStable,
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
            Self::Miniswap => "Miniswap",
            Self::Shibaswap => "Shibaswap",
            Self::Safeswap => "Safeswap",
            Self::Integral => "Integral",
            Self::Maverick => "Maverick",
            Self::Curve => "Curve",
            Self::LidoWstEth => "WstEth",
            Self::LidoStEth => "StEth",
            Self::RocketEth => "RocketEth",
        };
        write!(f, "{}", protocol_name)
    }
}

pub struct PoolWrapper {
    pub pool: Arc<dyn Pool>,
}

impl PartialOrd for PoolWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for PoolWrapper {}

impl Ord for PoolWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get_address().cmp(&other.get_address())
    }
}

impl Display for PoolWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.get_protocol(), self.get_address())
    }
}

impl Debug for PoolWrapper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.get_protocol(), self.get_address())
    }
}

impl Hash for PoolWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get_address().hash(state)
    }
}

impl PartialEq for PoolWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.pool.get_address() == other.pool.get_address()
    }
}

impl PoolWrapper {
    pub fn new(pool: Arc<dyn Pool>) -> Self {
        PoolWrapper { pool }
    }
}

impl Clone for PoolWrapper {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

impl Deref for PoolWrapper {
    type Target = dyn Pool;

    fn deref(&self) -> &Self::Target {
        self.pool.deref()
    }
}

impl<T: 'static + Pool + Clone> From<T> for PoolWrapper {
    fn from(pool: T) -> Self {
        Self { pool: Arc::new(pool) }
    }
}

pub trait Pool: Sync + Send {
    fn get_class(&self) -> PoolClass {
        PoolClass::Unknown
    }

    fn get_protocol(&self) -> PoolProtocol {
        PoolProtocol::Unknown
    }

    //fn clone_box(&self) -> Box<dyn Pool>;

    fn get_address(&self) -> Address;

    fn get_fee(&self) -> U256 {
        U256::ZERO
    }

    fn get_tokens(&self) -> Vec<Address> {
        Vec::new()
    }

    fn get_swap_directions(&self) -> Vec<(Address, Address)> {
        Vec::new()
    }

    fn calculate_out_amount(
        &self,
        state: &LoomInMemoryDB,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        in_amount: U256,
    ) -> Result<(U256, u64), ErrReport>;

    // returns (in_amount, gas_used)
    fn calculate_in_amount(
        &self,
        state: &LoomInMemoryDB,
        env: Env,
        token_address_from: &Address,
        token_address_to: &Address,
        out_amount: U256,
    ) -> Result<(U256, u64), ErrReport>;

    fn can_flash_swap(&self) -> bool;

    fn can_calculate_in_amount(&self) -> bool {
        true
    }

    fn get_encoder(&self) -> &dyn AbiSwapEncoder;

    fn get_read_only_cell_vec(&self) -> Vec<U256> {
        Vec::new()
    }

    fn get_state_required(&self) -> Result<RequiredState>;
}

pub struct DefaultAbiSwapEncoder {}

impl AbiSwapEncoder for DefaultAbiSwapEncoder {}

#[derive(Clone, Debug, PartialEq)]
pub enum PreswapRequirement {
    Unknown,
    Transfer(Address),
    Allowance,
    Callback,
    Base,
}

pub trait AbiSwapEncoder {
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
