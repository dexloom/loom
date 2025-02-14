#![allow(dead_code)]
pub use deploy::{MulticallerDeployer, DEFAULT_VIRTUAL_ADDRESS};
pub use multicaller_encoder::MulticallerEncoder;
pub use multicaller_encoder::MulticallerSwapEncoder;
pub use opcodes_encoder::{OpcodesEncoder, OpcodesEncoderV2};
pub use pool_abi_encoder::ProtocolABIEncoderV2;
pub use swapline_encoder::SwapLineEncoder;
pub use swapstep_encoder::SwapStepEncoder;

mod deploy;
mod multicaller_encoder;
mod opcodes_encoder;
mod opcodes_helpers;
pub mod pool_abi_encoder;
pub mod pool_opcodes_encoder;
mod swap_encoder;
mod swapline_encoder;
mod swapstep_encoder;
