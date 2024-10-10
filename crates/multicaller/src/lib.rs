#![allow(dead_code)]
pub use deploy::{MulticallerDeployer, DEFAULT_VIRTUAL_ADDRESS};
pub use helpers::EncoderHelper;
pub use multicaller_encoder::MulticallerEncoder;
pub use multicaller_encoder::MulticallerSwapEncoder;
pub use swapline_encoder::SwapLineEncoder;
pub use swapstep_encoder::SwapStepEncoder;

mod deploy;
mod helpers;
mod multicaller_encoder;
mod opcodes_encoder;
pub mod poolencoders;
mod swap_encoder;
mod swapline_encoder;
mod swapstep_encoder;
