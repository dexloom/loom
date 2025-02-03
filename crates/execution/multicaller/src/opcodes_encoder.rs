use std::ops::{BitOrAssign, Shl};

use alloy_primitives::{Bytes, U256};
use alloy_sol_types::SolInterface;
use eyre::{ErrReport, Result};
use lazy_static::lazy_static;

use loom_defi_abi::multicaller::IMultiCaller;
use loom_types_blockchain::{CallType, MulticallerCall, MulticallerCalls};

lazy_static! {
    static ref VALUE_CALL_SELECTOR: U256 = U256::from(0x7FFA);
    static ref CALCULATION_CALL_SELECTOR: U256 = U256::from(0x7FFB);
    static ref ZERO_VALUE_CALL_SELECTOR: U256 = U256::from(0x7FFC);
    static ref INTERNAL_CALL_SELECTOR: U256 = U256::from(0x7FFD);
    static ref STATIC_CALL_SELECTOR: U256 = U256::from(0x7FFE);
    static ref DELEGATE_CALL_SELECTOR: U256 = U256::from(0x7FFF);
}

pub struct OpcodesEncoderV2;

pub trait OpcodesEncoder {
    fn pack_do_calls(opcodes: &MulticallerCalls) -> Result<Bytes>;
    fn pack_do_calls_data(opcode: &MulticallerCalls) -> Result<Bytes>;
}

impl OpcodesEncoderV2 {
    fn encode_data_offset(is_relative: bool, stack_offset: u32, data_offset: u32, data_len: usize) -> u32 {
        let mut ret = if is_relative { 0x800000 } else { 0x0 };
        ret |= (stack_offset & 0x7) << 20;
        ret |= (data_len as u32 & 0xFF) << 12;
        ret |= data_offset & 0xFFF;
        ret
    }

    pub fn encode_call_stack(opcode: &MulticallerCall) -> u32 {
        if let Some(call_stack) = &opcode.call_stack {
            match opcode.call_type {
                CallType::InternalCall | CallType::CalculationCall => Self::encode_data_offset(
                    call_stack.is_relative,
                    call_stack.stack_offset,
                    call_stack.data_offset + 0xC,
                    call_stack.data_len,
                ),
                _ => Self::encode_data_offset(
                    call_stack.is_relative,
                    call_stack.stack_offset,
                    call_stack.data_offset + 0x20,
                    call_stack.data_len,
                ),
            }
        } else {
            0xFFFFFF
        }
    }

    pub fn encode_return_stack(opcode: &MulticallerCall) -> u32 {
        if let Some(return_stack) = &opcode.return_stack {
            Self::encode_data_offset(return_stack.is_relative, return_stack.stack_offset, return_stack.data_offset, return_stack.data_len)
        } else {
            0xFFFFFF
        }
    }

    fn pack_opcode(opcode: &MulticallerCall) -> Result<Vec<u8>> {
        let mut ret: Vec<u8> = Vec::new();
        let mut selector = U256::ZERO;
        //let mut selector_bytes_len = 0x20;
        let selector_call = match opcode.call_type {
            CallType::Call => {
                if opcode.value.is_none() {
                    *ZERO_VALUE_CALL_SELECTOR
                } else {
                    *VALUE_CALL_SELECTOR
                }
            }
            CallType::DelegateCall => *DELEGATE_CALL_SELECTOR,
            CallType::InternalCall => {
                //selector_bytes_len = 0xC;
                *INTERNAL_CALL_SELECTOR
            }
            CallType::StaticCall => *STATIC_CALL_SELECTOR,
            CallType::CalculationCall => {
                //selector_bytes_len = 0xC;
                *CALCULATION_CALL_SELECTOR
            }
            _ => {
                return Err(ErrReport::msg("WRONG_OPCODE"));
            }
        };

        if selector_call == *VALUE_CALL_SELECTOR && !opcode.value.unwrap_or_default().is_zero() {
            selector = opcode.value.unwrap_or_default().shl(0x10);
            selector.bitor_assign(U256::from(1).shl(96 - 1));
            selector.bitor_assign(U256::from(opcode.call_data.len()).shl(0));
        } else {
            selector.bitor_assign(selector_call.shl(80));
            selector.bitor_assign(U256::from(opcode.call_data.len()).shl(0));
            selector.bitor_assign(U256::from(Self::encode_call_stack(opcode)).shl(16));
            selector.bitor_assign(U256::from(Self::encode_return_stack(opcode)).shl(40));
        }

        let selector_bytes = selector.to_be_bytes::<32>();
        ret.append(&mut selector_bytes[20..32].to_vec());

        match opcode.call_type {
            CallType::CalculationCall | CallType::InternalCall => {}
            _ => {
                let mut address_bytes = opcode.to.to_vec();
                ret.append(&mut address_bytes);
            }
        }

        ret.append(&mut opcode.call_data.to_vec());

        Ok(ret)
    }
}

impl OpcodesEncoder for OpcodesEncoderV2 {
    fn pack_do_calls(opcodes: &MulticallerCalls) -> Result<Bytes> {
        let call_data = OpcodesEncoderV2::pack_do_calls_data(opcodes)?;
        let args = IMultiCaller::doCallsCall { data: call_data };
        let call = IMultiCaller::IMultiCallerCalls::doCalls(args);
        Ok(call.abi_encode().into())
    }

    fn pack_do_calls_data(opcodes: &MulticallerCalls) -> Result<Bytes> {
        let mut call_data: Vec<u8> = Vec::new();
        for o in opcodes.opcodes_vec.iter() {
            call_data.append(&mut OpcodesEncoderV2::pack_opcode(o)?);
        }
        Ok(call_data.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let buf = Bytes::from(vec![0x33, 0x33, 0x44, 0x55]);

        let mut opcode = MulticallerCall::new_internal_call(&buf);
        //let mut opcode = Opcode::new_internal_call(to, &Some(buf));
        opcode.set_call_stack(true, 0, 24, 0x20).set_return_stack(true, 1, 44, 0x20);

        let mut opcodes = MulticallerCalls::new();
        opcodes.add(opcode);

        let packed_bytes = OpcodesEncoderV2::pack_do_calls(&opcodes).unwrap();
        println!("{:?}", packed_bytes);
    }
}
