use crate::proto;
use crate::proto::tx_kind::Kind;
use alloy_primitives::{Address, BlockHash, B256};
use eyre::OptionExt;

impl TryFrom<&proto::BundleState> for reth::revm::db::BundleState {
    type Error = eyre::Error;

    fn try_from(bundle: &crate::proto::BundleState) -> Result<Self, Self::Error> {
        let ret = reth::revm::db::BundleState {
            state: bundle.state.iter().map(TryInto::try_into).collect::<eyre::Result<_>>()?,
            contracts: bundle
                .contracts
                .iter()
                .map(|contract| {
                    Ok((B256::try_from(contract.hash.as_slice())?, contract.bytecode.as_ref().ok_or_eyre("no bytecode")?.try_into()?))
                })
                .collect::<eyre::Result<_>>()?,
            reverts: reth::revm::db::states::reverts::Reverts::new(
                bundle
                    .reverts
                    .iter()
                    .map(|block_reverts| block_reverts.reverts.iter().map(TryInto::try_into).collect::<eyre::Result<_>>())
                    .collect::<eyre::Result<_>>()?,
            ),
            state_size: bundle.state_size as usize,
            reverts_size: bundle.reverts_size as usize,
        };
        Ok(ret)
    }
}

impl From<proto::TxKind> for Address {
    fn from(tx_kind: proto::TxKind) -> Self {
        match tx_kind.kind {
            Some(kind) => match kind {
                Kind::Create(_address) => Address::ZERO,
                Kind::Call(address) => Address::from_slice(address.as_slice()),
            },
            _ => Address::ZERO,
        }
    }
}

impl TryFrom<proto::SealedHeader> for reth::primitives::SealedHeader {
    type Error = eyre::Error;

    fn try_from(sealed_header: proto::SealedHeader) -> Result<Self, Self::Error> {
        let header = sealed_header.header.as_ref().ok_or_eyre("no header")?;
        Ok(reth::primitives::SealedHeader::new(header.try_into()?, BlockHash::try_from(sealed_header.hash.as_slice())?))
    }
}
