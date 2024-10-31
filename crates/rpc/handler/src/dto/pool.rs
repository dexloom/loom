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
impl From<loom_types_entities::PoolClass> for PoolClass {
    fn from(pool_class: loom_types_entities::PoolClass) -> Self {
        match pool_class {
            loom_types_entities::PoolClass::Unknown => PoolClass::Unknown,
            loom_types_entities::PoolClass::UniswapV2 => PoolClass::UniswapV2,
            loom_types_entities::PoolClass::UniswapV3 => PoolClass::UniswapV3,
            loom_types_entities::PoolClass::Curve => PoolClass::Curve,
            loom_types_entities::PoolClass::LidoStEth => PoolClass::LidoStEth,
            loom_types_entities::PoolClass::LidoWstEth => PoolClass::LidoWstEth,
            loom_types_entities::PoolClass::RocketPool => PoolClass::RocketPool,
            loom_types_entities::PoolClass::Custom(id) => PoolClass::Custom(id),
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
impl From<loom_types_entities::PoolProtocol> for PoolProtocol {
    fn from(protocol: loom_types_entities::PoolProtocol) -> Self {
        match protocol {
            loom_types_entities::PoolProtocol::Unknown => PoolProtocol::Unknown,
            loom_types_entities::PoolProtocol::UniswapV2 => PoolProtocol::UniswapV2,
            loom_types_entities::PoolProtocol::UniswapV2Like => PoolProtocol::UniswapV2Like,
            loom_types_entities::PoolProtocol::NomiswapStable => PoolProtocol::NomiswapStable,
            loom_types_entities::PoolProtocol::Sushiswap => PoolProtocol::Sushiswap,
            loom_types_entities::PoolProtocol::SushiswapV3 => PoolProtocol::SushiswapV3,
            loom_types_entities::PoolProtocol::DooarSwap => PoolProtocol::DooarSwap,
            loom_types_entities::PoolProtocol::Safeswap => PoolProtocol::Safeswap,
            loom_types_entities::PoolProtocol::Miniswap => PoolProtocol::Miniswap,
            loom_types_entities::PoolProtocol::Shibaswap => PoolProtocol::Shibaswap,
            loom_types_entities::PoolProtocol::UniswapV3 => PoolProtocol::UniswapV3,
            loom_types_entities::PoolProtocol::UniswapV3Like => PoolProtocol::UniswapV3Like,
            loom_types_entities::PoolProtocol::PancakeV3 => PoolProtocol::PancakeV3,
            loom_types_entities::PoolProtocol::Integral => PoolProtocol::Integral,
            loom_types_entities::PoolProtocol::Maverick => PoolProtocol::Maverick,
            loom_types_entities::PoolProtocol::Curve => PoolProtocol::Curve,
            loom_types_entities::PoolProtocol::LidoStEth => PoolProtocol::LidoStEth,
            loom_types_entities::PoolProtocol::LidoWstEth => PoolProtocol::LidoWstEth,
            loom_types_entities::PoolProtocol::RocketEth => PoolProtocol::RocketEth,
            loom_types_entities::PoolProtocol::OgPepe => PoolProtocol::OgPepe,
            loom_types_entities::PoolProtocol::Custom(id) => PoolProtocol::Custom(id),
        }
    }
}

impl From<&PoolProtocol> for loom_types_entities::PoolProtocol {
    fn from(protocol: &PoolProtocol) -> Self {
        match protocol {
            PoolProtocol::Unknown => loom_types_entities::PoolProtocol::Unknown,
            PoolProtocol::UniswapV2 => loom_types_entities::PoolProtocol::UniswapV2,
            PoolProtocol::UniswapV2Like => loom_types_entities::PoolProtocol::UniswapV2Like,
            PoolProtocol::NomiswapStable => loom_types_entities::PoolProtocol::NomiswapStable,
            PoolProtocol::Sushiswap => loom_types_entities::PoolProtocol::Sushiswap,
            PoolProtocol::SushiswapV3 => loom_types_entities::PoolProtocol::SushiswapV3,
            PoolProtocol::DooarSwap => loom_types_entities::PoolProtocol::DooarSwap,
            PoolProtocol::Safeswap => loom_types_entities::PoolProtocol::Safeswap,
            PoolProtocol::Miniswap => loom_types_entities::PoolProtocol::Miniswap,
            PoolProtocol::Shibaswap => loom_types_entities::PoolProtocol::Shibaswap,
            PoolProtocol::UniswapV3 => loom_types_entities::PoolProtocol::UniswapV3,
            PoolProtocol::UniswapV3Like => loom_types_entities::PoolProtocol::UniswapV3Like,
            PoolProtocol::PancakeV3 => loom_types_entities::PoolProtocol::PancakeV3,
            PoolProtocol::Integral => loom_types_entities::PoolProtocol::Integral,
            PoolProtocol::Maverick => loom_types_entities::PoolProtocol::Maverick,
            PoolProtocol::Curve => loom_types_entities::PoolProtocol::Curve,
            PoolProtocol::LidoStEth => loom_types_entities::PoolProtocol::LidoStEth,
            PoolProtocol::LidoWstEth => loom_types_entities::PoolProtocol::LidoWstEth,
            PoolProtocol::RocketEth => loom_types_entities::PoolProtocol::RocketEth,
            PoolProtocol::OgPepe => loom_types_entities::PoolProtocol::OgPepe,
            PoolProtocol::Custom(id) => loom_types_entities::PoolProtocol::Custom(*id),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MarketStats {
    pub total_pools: usize,
}
