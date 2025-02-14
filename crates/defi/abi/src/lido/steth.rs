use alloy::sol;

sol! {
    #[derive(Debug, PartialEq, Eq)]
    interface IStEth {
        event TransferShares(
            address indexed from,
            address indexed to,
            uint256 sharesValue
        );
        event SharesBurnt(
            address indexed account,
            uint256 preRebaseTokenAmount,
            uint256 postRebaseTokenAmount,
            uint256 sharesAmount
        );

        function totalSupply() external view returns (uint256);
        function getTotalPooledEther() external view returns (uint256);
        function balanceOf(address _account) external view returns (uint256);
        function getTotalShares() external view returns (uint256);
        function sharesOf(address _account) external view returns (uint256);
        function getSharesByPooledEth(uint256 _ethAmount) public view returns (uint256);
        function getPooledEthByShares(uint256 _sharesAmount) public view returns (uint256);

        function submit(address _referral) external payable returns (uint256);
        function transferShares(address _recipient, uint256 _sharesAmount) external returns (uint256);
        function transferSharesFrom(address _sender, address _recipient, uint256 _sharesAmount) external returns (uint256);

    }
}
