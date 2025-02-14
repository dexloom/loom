use alloy::sol;

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveFactory {
        function pool_list(uint256) external view returns (address);
        function pool_count() external view returns (uint256);
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveAddressProvider {
        function get_address(uint256) external view returns (address);
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveCommon {
        function coins(uint256) external view returns (bytes);
        function balances(uint256) external view returns (uint256);
        function get_balances() external view returns (bytes);
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveCommonI128 {
        function coins(int128) external view returns (bytes);
        function balances(int128) external view returns (uint256);
    }
}
