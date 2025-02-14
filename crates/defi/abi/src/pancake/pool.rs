use alloy::sol;

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IPancakeV3Pool {

        function factory() external view returns (address);

        function token0() external view returns (address);

        function token1() external view returns (address);

        function fee() external view returns (uint24);

        function tickSpacing() external view returns (int24);

        function maxLiquidityPerTick() external view returns (uint128);

        function slot0()
            external
            view
            returns (
                uint160 sqrtPriceX96,
                int24 tick,
                uint16 observationIndex,
                uint16 observationCardinality,
                uint16 observationCardinalityNext,
                uint32 feeProtocol,
                bool unlocked
            );

        function feeGrowthGlobal0X128() external view returns (uint256);

        function feeGrowthGlobal1X128() external view returns (uint256);

        function protocolFees() external view returns (uint128 token0, uint128 token1);

        function liquidity() external view returns (uint128);

        function ticks(int24 tick)
            external
            view
            returns (
                uint128 liquidityGross,
                int128 liquidityNet,
                uint256 feeGrowthOutside0X128,
                uint256 feeGrowthOutside1X128,
                int56 tickCumulativeOutside,
                uint160 secondsPerLiquidityOutsideX128,
                uint32 secondsOutside,
                bool initialized
            );

        function tickBitmap(int16 wordPosition) external view returns (uint256);

        function positions(bytes32 key)
            external
            view
            returns (
                uint128 _liquidity,
                uint256 feeGrowthInside0LastX128,
                uint256 feeGrowthInside1LastX128,
                uint128 tokensOwed0,
                uint128 tokensOwed1
            );

        function observations(uint256 index)
            external
            view
            returns (
                uint32 blockTimestamp,
                int56 tickCumulative,
                uint160 secondsPerLiquidityCumulativeX128,
                bool initialized
            );
    }
}
