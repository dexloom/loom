#![allow(dead_code)]
pub use abi_encoders::ProtocolABIEncoderV2;
pub use deploy::{MulticallerDeployer, DEFAULT_VIRTUAL_ADDRESS};
pub use helpers::EncoderHelper;
pub use multicaller_encoder::MulticallerEncoder;
pub use multicaller_encoder::MulticallerSwapEncoder;
pub use swapline_encoder::SwapLineEncoder;
pub use swapstep_encoder::SwapStepEncoder;

mod abi_encoders;
mod deploy;
mod helpers;
mod multicaller_encoder;
mod opcodes_encoder;
mod swap_encoder;
mod swap_encoders;
mod swapline_encoder;
mod swapstep_encoder;
