//! # Ethers Flashbots
//!
//! Provides an [ethers](https://docs.rs/ethers) compatible middleware for submitting
//! [Flashbots](https://docs.flashbots.net) bundles.
//!
//! In addition to leveraging the standard Ethers middleware API ([`send_transaction`][ethers::providers::Middleware::send_transaction]),
//! custom bundles can be crafted, simulated and submitted.
pub use body::make_signed_body;
pub use bundle::{BundleHash, BundleRequest, BundleTransaction, SimulatedBundle, SimulatedTransaction};
pub use jsonrpc::SendBundleResponseType;
pub use middleware::{FlashbotsMiddleware, FlashbotsMiddlewareError};
pub use relay::{Relay, RelayError};

mod bundle;

mod middleware;

mod jsonrpc;
mod relay;

mod body;
mod utils;
