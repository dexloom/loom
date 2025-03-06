use alloy::primitives::ChainId;
use alloy_chains::{Chain, NamedChain};
use eyre::{eyre, OptionExt, Result};
use loom_defi_address_book::{TokenAddressArbitrum, TokenAddressBase, TokenAddressEth};
use loom_types_entities::{Market, Token};

pub fn add_default_tokens_to_market(market: &mut Market, chain_id: ChainId) -> Result<()> {
    match Chain::from_id(chain_id).named().ok_or_eyre("NO_NAMED_CHAIN")? {
        NamedChain::Mainnet => {
            let weth_token = Token::new_with_data(TokenAddressEth::WETH, Some("WETH".to_string()), None, Some(18), true, false);
            let usdc_token = Token::new_with_data(TokenAddressEth::USDC, Some("USDC".to_string()), None, Some(6), true, false);
            let usdt_token = Token::new_with_data(TokenAddressEth::USDT, Some("USDT".to_string()), None, Some(6), true, false);
            let dai_token = Token::new_with_data(TokenAddressEth::DAI, Some("DAI".to_string()), None, Some(18), true, false);
            let wbtc_token = Token::new_with_data(TokenAddressEth::WBTC, Some("WBTC".to_string()), None, Some(8), true, false);
            let threecrv_token = Token::new_with_data(TokenAddressEth::THREECRV, Some("3Crv".to_string()), None, Some(18), false, true);

            market.add_token(weth_token);
            market.add_token(usdc_token);
            market.add_token(usdt_token);
            market.add_token(dai_token);
            market.add_token(wbtc_token);
            market.add_token(threecrv_token);
        }
        NamedChain::Arbitrum => {
            let weth_token = Token::new_with_data(TokenAddressArbitrum::WETH, Some("WETH".to_string()), None, Some(18), true, false);
            let wbtc_token = Token::new_with_data(TokenAddressArbitrum::WBTC, Some("WBTC".to_string()), None, Some(8), true, false);
            let usdc_token = Token::new_with_data(TokenAddressArbitrum::USDC, Some("USDC".to_string()), None, Some(6), true, false);
            let usdt_token = Token::new_with_data(TokenAddressArbitrum::USDT, Some("USDT".to_string()), None, Some(6), true, false);
            let dai_token = Token::new_with_data(TokenAddressEth::DAI, Some("DAI".to_string()), None, Some(18), true, false);

            market.add_token(weth_token);
            market.add_token(wbtc_token);
            market.add_token(usdc_token);
            market.add_token(usdt_token);
            market.add_token(dai_token);
        }
        NamedChain::Base => {
            let weth_token = Token::new_with_data(TokenAddressBase::WETH, Some("WETH".to_string()), None, Some(18), true, false);
            let usdc_token = Token::new_with_data(TokenAddressBase::USDC, Some("USDC".to_string()), None, Some(6), true, false);

            market.add_token(weth_token);
            market.add_token(usdc_token);
        }
        _ => return Err(eyre!("CHAIN_TOKENS_NOT_LOADED")),
    }
    Ok(())
}
