use alloy::sol;

sol! {

    #[derive(Debug, PartialEq, Eq)]
    struct BinInfo {
        uint128 id;
        uint8 kind;
        int32 lowerTick;
        uint128 reserveA;
        uint128 reserveB;
        uint128 mergeId;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct BinState {
        uint128 reserveA;
        uint128 reserveB;
        uint128 mergeBinBalance;
        uint128 mergeId;
        uint128 totalSupply;
        uint8 kind;
        int32 lowerTick;
    }


    #[sol(abi=true,rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IMaverickQuoter {
        function calculateSwap(address pool, uint128 amount, bool tokenAIn, bool exactOutput, uint256 sqrtPriceLimit) external returns (uint256 returnAmount);
        function calculateMultihopSwap(bytes memory path, uint256 amount, bool exactOutput) external returns (uint256 returnAmount);

        function getActiveBins(address pool, uint128 startBinIndex, uint128 endBinIndex) external view returns (BinInfo[] memory bins);

        function getBinDepth(address pool, uint128 binId) external view returns (uint256 depth);

        function getSqrtPrice(address pool) external view returns (uint256 sqrtPrice);

        function getBinsAtTick(address pool, int32 tick) external view returns (BinState[] memory bins);

        function activeTickLiquidity(address pool) external view returns (uint256 sqrtPrice, uint256 liquidity, uint256 reserveA, uint256 reserveB);

        function tickLiquidity(address pool, int32 tick) external view returns (uint256 sqrtPrice, uint256 liquidity, uint256 reserveA, uint256 reserveB);

    }
}
