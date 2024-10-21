pub use client::ExExClient;

pub mod codec;
pub mod codec_extra;

mod client;
mod helpers;

pub mod proto {
    tonic::include_proto!("exex");
}
