use alloy::sol;

sol! {
    #[derive(Debug, PartialEq, Eq)]
    interface IWStEth {
        function getWstETHByStETH(uint256 stETHAmount) external view returns (uint256);
        function getStETHByWstETH(uint256 wstETHAmount) external view returns (uint256);
        function stEthPerToken() returns (uint256);
        function tokensPerStEth() returns (uint256);
        function wrap(uint256 stETHAmount) returns (uint256);
        function unwrap(uint256 wstETHAmount) returns (uint256);
    }
}
