use alloy::sol;

sol! {
    #[derive(Debug, PartialEq, Eq)]
    struct AddLiquidityParamsQuoter {
        uint8 kind;
        int32[] ticks;
        uint128[] amounts;
    }


    #[sol(abi=true,rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IMaverickV2Quoter {
        error QuoterInvalidSwap();
        error QuoterInvalidAddLiquidity();

        /**
         * @notice Calculates a swap on a MaverickV2Pool and returns the resulting
         * amount and estimated gas.  The gas estimate is only a rough estimate and
         * may not match a swap's gas.
         * @param pool The MaverickV2Pool to swap on.
         * @param amount The input amount.
         * @param tokenAIn Indicates if token A is the input token.
         * @param exactOutput Indicates if the amount is the output amount (true)
         * or input amount (false). If the tickLimit is reached, the full value of
         * the exactOutput may not be returned because the pool will stop swapping
         * before the whole order is filled.
         * @param tickLimit The tick limit for the swap. Once the swap lands in
         * this tick, it will stop and return the output amount swapped up to that
         * tick.
         */
        function calculateSwap(
            address pool,
            uint128 amount,
            bool tokenAIn,
            bool exactOutput,
            int32 tickLimit
        ) external returns (uint256 amountIn, uint256 amountOut, uint256 gasEstimate);

        /**
         * @notice Calculates a multihop swap and returns the resulting amount and
         * estimated gas. The gas estimate is only a rough estimate and
         * may not match a swap's gas.
         * @param path The path of pools to swap through. Path is given by an
         * packed array of (pool, tokenAIn) tuples. So each step in the path is 160
         * + 8 = 168 bits of data. e.g. path = abi.encodePacked(pool1, true, pool2, false);
         * @param amount The input amount.
         * @param exactOutput A boolean indicating if exact output is required.
         */
        function calculateMultiHopSwap(
            bytes memory path,
            uint256 amount,
            bool exactOutput
        ) external returns (uint256 returnAmount, uint256 gasEstimate);

        /**
         * @notice Computes the token amounts required for a given set of
         * addLiquidity parameters. The gas estimate is only a rough estimate and
         * may not match a add's gas.
         */
        function calculateAddLiquidity(
            address pool,
            AddLiquidityParamsQuoter calldata params
        ) external returns (uint256 amountA, uint256 amountB, uint256 gasEstimate);

        /**
         * @notice Pool's sqrt price.
         */
        function poolSqrtPrice(address pool) external view returns (uint256 sqrtPrice);
    }

}
