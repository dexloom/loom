use alloy_primitives::{Address, Bytes, U256};
use log::debug;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpcodeType {
    Unknown,
    Call,
    DelegateCall,
    StaticCall,
    InternalCall,
    CalculationCall,
}


#[derive(Clone, Debug)]
pub struct Opcode {
    pub opcode_type: OpcodeType,
    pub call_data: Bytes,
    pub to: Address,
    pub value: Option<U256>,
    pub call_stack: u32,
    pub return_stack: u32,
}


impl Opcode {
    pub fn new(opcode_type: OpcodeType, to: Address, call_data: &Bytes, value: Option<U256>) -> Opcode {
        Opcode {
            opcode_type,
            to,
            call_data: call_data.clone(),
            value,
            call_stack: 0,
            return_stack: 0,
        }
    }

    pub fn new_call(to: Address, call_data: &Bytes) -> Opcode {
        Opcode::new(OpcodeType::Call, to, call_data, None)
    }
    pub fn new_call_with_value(to: Address, call_data: &Bytes, value: U256) -> Opcode {
        Opcode::new(OpcodeType::Call, to, call_data, Some(value))
    }
    pub fn new_internal_call(call_data: &Bytes) -> Opcode {
        Opcode::new(OpcodeType::InternalCall, Address::ZERO, call_data, None)
    }

    pub fn new_calculation_call(call_data: &Bytes) -> Opcode {
        Opcode::new(OpcodeType::CalculationCall, Address::ZERO, call_data, None)
    }

    pub fn new_delegate_call(to: Address, call_data: &Bytes) -> Opcode {
        Opcode::new(OpcodeType::DelegateCall, to, call_data, None)
    }

    pub fn new_static_call(to: Address, call_data: &Bytes) -> Opcode {
        Opcode::new(OpcodeType::StaticCall, to, call_data, None)
    }


    fn encode_data_offset(is_relative: bool, stack_offset: u32, data_offset: u32, data_len: usize) -> u32 {
        let mut ret = if is_relative { 0x800000 } else { 0x0 };
        ret |= (stack_offset & 0x7) << 20;
        ret |= (data_len as u32 & 0xFF) << 12;
        ret |= data_offset & 0xFFF;
        ret
    }

    pub fn set_call_stack(&mut self, is_relative: bool, stack_offset: u32, data_offset: u32, data_len: usize) -> &mut Self {
        self.call_stack =
            match self.opcode_type {
                OpcodeType::InternalCall | OpcodeType::CalculationCall => {
                    Opcode::encode_data_offset(is_relative, stack_offset, data_offset + 0xC, data_len)
                }
                _ => {
                    Opcode::encode_data_offset(is_relative, stack_offset, data_offset + 0x20, data_len)
                }
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
        self.return_stack = Opcode::encode_data_offset(is_relative, stack_offset, data_offset, data_len);
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
pub struct Opcodes {
    pub opcodes_vec: Vec<Opcode>,
}

impl Opcodes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn log(&self) {
        for (i, o) in self.opcodes_vec.iter().enumerate() {
            debug!("{} {:?}", i, o);
        }
    }

    pub fn add(&mut self, opcode: Opcode) -> &mut Self {
        self.opcodes_vec.push(opcode);
        self
    }

    pub fn insert(&mut self, opcode: Opcode) -> &mut Self {
        self.opcodes_vec.insert(0, opcode);
        self
    }


    pub fn merge(&mut self, opcodes: Opcodes) -> &mut Self {
        self.opcodes_vec.extend(opcodes.opcodes_vec);
        self
    }


    pub fn get(&self, idx: usize) -> Option<&Opcode> {
        self.opcodes_vec.get(idx)
    }
}
