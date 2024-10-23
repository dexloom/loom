use alloy_primitives::{Address, U256};

use serde::{Deserialize, Serialize};
use utoipa::openapi::schema::SchemaType;
use utoipa::openapi::{Array, Object, ToArray, Type};
use utoipa::PartialSchema;
use utoipa::{schema, ToSchema};

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolResponse {
    pub pools: Vec<Pool>,
    pub total: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PoolDetailsResponse {
    #[schema(schema_with = String::schema)]
    pub address: Address,
    pub protocol: PoolProtocol,
    pub pool_class: PoolClass,
    #[schema(schema_with = String::schema)]
    pub fee: U256,
    #[schema(schema_with = array_of_strings)]
    pub tokens: Vec<Address>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Pool {
    #[schema(schema_with = String::schema)]
    pub address: Address,
    #[schema(schema_with = String::schema)]
    pub fee: U256,
    #[schema(schema_with = array_of_strings)]
    pub tokens: Vec<Address>,
    pub protocol: PoolProtocol,
    pub pool_class: PoolClass,
}

pub fn array_of_strings() -> Array {
    Object::with_type(SchemaType::Type(Type::String)).to_array()
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PoolClass {
    Unknown,
    UniswapV2,
    UniswapV3,
    Curve,
    LidoStEth,
    LidoWstEth,
    RocketPool,
    Custom(u64),
}
impl From<defi_entities::PoolClass> for PoolClass {
    fn from(pool_class: defi_entities::PoolClass) -> Self {
        match pool_class {
            defi_entities::PoolClass::Unknown => PoolClass::Unknown,
            defi_entities::PoolClass::UniswapV2 => PoolClass::UniswapV2,
            defi_entities::PoolClass::UniswapV3 => PoolClass::UniswapV3,
            defi_entities::PoolClass::Curve => PoolClass::Curve,
            defi_entities::PoolClass::LidoStEth => PoolClass::LidoStEth,
            defi_entities::PoolClass::LidoWstEth => PoolClass::LidoWstEth,
            defi_entities::PoolClass::RocketPool => PoolClass::RocketPool,
            defi_entities::PoolClass::Custom(id) => PoolClass::Custom(id),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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
    OgPepe,
    Custom(u64),
}
impl From<defi_entities::PoolProtocol> for PoolProtocol {
    fn from(protocol: defi_entities::PoolProtocol) -> Self {
        match protocol {
            defi_entities::PoolProtocol::Unknown => PoolProtocol::Unknown,
            defi_entities::PoolProtocol::UniswapV2 => PoolProtocol::UniswapV2,
            defi_entities::PoolProtocol::UniswapV2Like => PoolProtocol::UniswapV2Like,
            defi_entities::PoolProtocol::NomiswapStable => PoolProtocol::NomiswapStable,
            defi_entities::PoolProtocol::Sushiswap => PoolProtocol::Sushiswap,
            defi_entities::PoolProtocol::SushiswapV3 => PoolProtocol::SushiswapV3,
            defi_entities::PoolProtocol::DooarSwap => PoolProtocol::DooarSwap,
            defi_entities::PoolProtocol::Safeswap => PoolProtocol::Safeswap,
            defi_entities::PoolProtocol::Miniswap => PoolProtocol::Miniswap,
            defi_entities::PoolProtocol::Shibaswap => PoolProtocol::Shibaswap,
            defi_entities::PoolProtocol::UniswapV3 => PoolProtocol::UniswapV3,
            defi_entities::PoolProtocol::UniswapV3Like => PoolProtocol::UniswapV3Like,
            defi_entities::PoolProtocol::PancakeV3 => PoolProtocol::PancakeV3,
            defi_entities::PoolProtocol::Integral => PoolProtocol::Integral,
            defi_entities::PoolProtocol::Maverick => PoolProtocol::Maverick,
            defi_entities::PoolProtocol::Curve => PoolProtocol::Curve,
            defi_entities::PoolProtocol::LidoStEth => PoolProtocol::LidoStEth,
            defi_entities::PoolProtocol::LidoWstEth => PoolProtocol::LidoWstEth,
            defi_entities::PoolProtocol::RocketEth => PoolProtocol::RocketEth,
            defi_entities::PoolProtocol::OgPepe => PoolProtocol::OgPepe,
            defi_entities::PoolProtocol::Custom(id) => PoolProtocol::Custom(id),
        }
    }
}

impl From<&PoolProtocol> for defi_entities::PoolProtocol {
    fn from(protocol: &PoolProtocol) -> Self {
        match protocol {
            PoolProtocol::Unknown => defi_entities::PoolProtocol::Unknown,
            PoolProtocol::UniswapV2 => defi_entities::PoolProtocol::UniswapV2,
            PoolProtocol::UniswapV2Like => defi_entities::PoolProtocol::UniswapV2Like,
            PoolProtocol::NomiswapStable => defi_entities::PoolProtocol::NomiswapStable,
            PoolProtocol::Sushiswap => defi_entities::PoolProtocol::Sushiswap,
            PoolProtocol::SushiswapV3 => defi_entities::PoolProtocol::SushiswapV3,
            PoolProtocol::DooarSwap => defi_entities::PoolProtocol::DooarSwap,
            PoolProtocol::Safeswap => defi_entities::PoolProtocol::Safeswap,
            PoolProtocol::Miniswap => defi_entities::PoolProtocol::Miniswap,
            PoolProtocol::Shibaswap => defi_entities::PoolProtocol::Shibaswap,
            PoolProtocol::UniswapV3 => defi_entities::PoolProtocol::UniswapV3,
            PoolProtocol::UniswapV3Like => defi_entities::PoolProtocol::UniswapV3Like,
            PoolProtocol::PancakeV3 => defi_entities::PoolProtocol::PancakeV3,
            PoolProtocol::Integral => defi_entities::PoolProtocol::Integral,
            PoolProtocol::Maverick => defi_entities::PoolProtocol::Maverick,
            PoolProtocol::Curve => defi_entities::PoolProtocol::Curve,
            PoolProtocol::LidoStEth => defi_entities::PoolProtocol::LidoStEth,
            PoolProtocol::LidoWstEth => defi_entities::PoolProtocol::LidoWstEth,
            PoolProtocol::RocketEth => defi_entities::PoolProtocol::RocketEth,
            PoolProtocol::OgPepe => defi_entities::PoolProtocol::OgPepe,
            PoolProtocol::Custom(id) => defi_entities::PoolProtocol::Custom(*id),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MarketStats {
    pub total_pools: usize,
}
