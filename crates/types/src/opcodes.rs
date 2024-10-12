use alloy_primitives::{Address, Bytes, U256};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use tracing::debug;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallType {
    Unknown,
    Call,
    DelegateCall,
    StaticCall,
    InternalCall,
    CalculationCall,
}

#[derive(Clone, Debug)]
pub struct DataOffset(u32);

impl DataOffset {
    pub fn empty() -> Self {
        DataOffset(0)
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    pub fn value(&self) -> u32 {
        self.0 & 0xFFFFFF
    }

    pub fn encode(is_relative: bool, stack_offset: u32, data_offset: u32, data_len: usize) -> DataOffset {
        let mut ret = if is_relative { 0x800000 } else { 0x0 };
        ret |= (stack_offset & 0x7) << 20;
        ret |= (data_len as u32 & 0xFF) << 12;
        ret |= data_offset & 0xFFF;
        DataOffset(ret)
    }

    pub fn is_relative(&self) -> u32 {
        self.0 >> 23 & 1
    }

    pub fn stack_offset(&self) -> u32 {
        self.0 >> 20 & 0x7
    }

    pub fn data_len(&self) -> u32 {
        self.0 >> 12 & 0xFF
    }

    pub fn data_offset(&self) -> u32 {
        self.0 & 0xFFF
    }
}

impl Deref for DataOffset {
    type Target = u32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for DataOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            write!(f, "empty")
        } else {
            write!(f, "rel {} stack {} len {} offs {} ", self.is_relative(), self.stack_offset(), self.data_len(), self.data_offset())
        }
    }
}

#[derive(Clone, Debug)]
pub struct MulticallerCall {
    pub call_type: CallType,
    pub call_data: Bytes,
    pub to: Address,
    pub value: Option<U256>,
    pub call_stack: DataOffset,
    pub return_stack: DataOffset,
}

impl Display for MulticallerCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} to {} value {} call [{}] ret [{}] data {:?}",
            self.call_type,
            self.to,
            self.value.unwrap_or_default(),
            self.call_stack,
            self.return_stack,
            self.call_data
        )
    }
}

impl MulticallerCall {
    pub fn new(opcode_type: CallType, to: Address, call_data: &Bytes, value: Option<U256>) -> MulticallerCall {
        MulticallerCall {
            call_type: opcode_type,
            to,
            call_data: call_data.clone(),
            value,
            call_stack: DataOffset::empty(),
            return_stack: DataOffset::empty(),
        }
    }

    pub fn new_call(to: Address, call_data: &Bytes) -> MulticallerCall {
        MulticallerCall::new(CallType::Call, to, call_data, None)
    }
    pub fn new_call_with_value(to: Address, call_data: &Bytes, value: U256) -> MulticallerCall {
        MulticallerCall::new(CallType::Call, to, call_data, Some(value))
    }
    pub fn new_internal_call(call_data: &Bytes) -> MulticallerCall {
        MulticallerCall::new(CallType::InternalCall, Address::ZERO, call_data, None)
    }

    pub fn new_calculation_call(call_data: &Bytes) -> MulticallerCall {
        MulticallerCall::new(CallType::CalculationCall, Address::ZERO, call_data, None)
    }

    pub fn new_delegate_call(to: Address, call_data: &Bytes) -> MulticallerCall {
        MulticallerCall::new(CallType::DelegateCall, to, call_data, None)
    }

    pub fn new_static_call(to: Address, call_data: &Bytes) -> MulticallerCall {
        MulticallerCall::new(CallType::StaticCall, to, call_data, None)
    }

    fn encode_data_offset(is_relative: bool, stack_offset: u32, data_offset: u32, data_len: usize) -> DataOffset {
        let mut ret = if is_relative { 0x800000 } else { 0x0 };
        ret |= (stack_offset & 0x7) << 20;
        ret |= (data_len as u32 & 0xFF) << 12;
        ret |= data_offset & 0xFFF;
        DataOffset(ret)
    }

    pub fn set_call_stack(&mut self, is_relative: bool, stack_offset: u32, data_offset: u32, data_len: usize) -> &mut Self {
        self.call_stack = match self.call_type {
            CallType::InternalCall | CallType::CalculationCall => {
                MulticallerCall::encode_data_offset(is_relative, stack_offset, data_offset + 0xC, data_len)
            }
            _ => MulticallerCall::encode_data_offset(is_relative, stack_offset, data_offset + 0x20, data_len),
        };
        self
    }

    /*
    pub fn set_uniswap2_swap_out_amount_stack(&mut self, is_relative : bool, stack_offset : u32, zeroforone : bool) -> &mut Self{
        self.call_stack = if zeroforone {
            Opcode::encode_data_offset(is_relative, stack_offset, 0x20+ 0x24, 0x20)
        }else{
            Opcode::encode_data_offset(is_relative, stack_offset, 0x20+ 0x04, 0x20)
        };

        self
    }

     */

    pub fn set_return_stack(&mut self, is_relative: bool, stack_offset: u32, data_offset: u32, data_len: usize) -> &mut Self {
        self.return_stack = MulticallerCall::encode_data_offset(is_relative, stack_offset, data_offset, data_len);
        self
    }

    /*
    pub fn set_uniswap3_return_stack(&mut self, is_relative : bool, stack_offset : u32, zeroforone : bool) -> &mut Self{
        self.return_stack = if zeroforone {
            Opcode::encode_data_offset(is_relative, stack_offset, 0x20, 0x20)
        }else{
            Opcode::encode_data_offset(is_relative, stack_offset, 0x0, 0x20)
        };
        self
    }
     */
}

#[derive(Clone, Debug, Default)]
pub struct MulticallerCalls {
    pub opcodes_vec: Vec<MulticallerCall>,
}

impl MulticallerCalls {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn log(&self) {
        for (i, o) in self.opcodes_vec.iter().enumerate() {
            debug!("{} {:?}", i, o);
        }
    }

    pub fn add(&mut self, opcode: MulticallerCall) -> &mut Self {
        debug!(%opcode, "add");
        self.opcodes_vec.push(opcode);
        self
    }

    pub fn insert(&mut self, opcode: MulticallerCall) -> &mut Self {
        self.opcodes_vec.insert(0, opcode);
        self
    }

    pub fn merge(&mut self, opcodes: MulticallerCalls) -> &mut Self {
        self.opcodes_vec.extend(opcodes.opcodes_vec);
        self
    }

    pub fn get(&self, idx: usize) -> Option<&MulticallerCall> {
        self.opcodes_vec.get(idx)
    }
}
