use std::str::FromStr;

use alloy_primitives::{Address, U256, U64};
use serde::{de, Deserialize};
use serde_json::Value;

pub fn deserialize_u64<'de, D>(deserializer: D) -> Result<U64, D::Error>
where
    D: de::Deserializer<'de>,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => {
            if s.as_str() == "0x" {
                return Ok(U64::ZERO);
            }

            if s.as_str().starts_with("0x") {
                U64::from_str_radix(s.as_str(), 16).map_err(de::Error::custom)?
            } else {
                U64::from_str_radix(s.as_str(), 10).map_err(de::Error::custom)?
            }
        }
        Value::Number(num) => U64::from(num.as_u64().ok_or_else(|| de::Error::custom("Invalid number"))?),
        _ => return Err(de::Error::custom("wrong type")),
    })
}

pub fn deserialize_u256<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: de::Deserializer<'de>,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => {
            if s.as_str() == "0x" {
                return Ok(U256::ZERO);
            }

            if s.as_str().starts_with("0x") {
                U256::from_str_radix(s.as_str(), 16).map_err(de::Error::custom)?
            } else {
                U256::from_str_radix(s.as_str(), 10).map_err(de::Error::custom)?
            }
        }
        Value::Number(num) => U256::from(num.as_u64().ok_or_else(|| de::Error::custom("Invalid number"))?),
        _ => return Err(de::Error::custom("wrong type")),
    })
}

pub fn deserialize_optional_h160<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
where
    D: de::Deserializer<'de>,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => {
            if s.as_str() == "0x" {
                return Ok(None);
            }

            Some(Address::from_str(s.as_str()).map_err(de::Error::custom)?)
        }
        _ => return Err(de::Error::custom("expected a hexadecimal string")),
    })
}
