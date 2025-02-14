use alloy::sol;

sol! {
    #[derive(Debug, PartialEq, Eq)]
    interface ITickLens {
        struct PopulatedTick {
            int24 tick;
            int128 liquidityNet;
            uint128 liquidityGross;
        }

        function getPopulatedTicksInWord(address pool, int16 tickBitmapIndex)
            external
            view
            returns (PopulatedTick[] memory populatedTicks);
    }
}
