use alloy::sol;

sol! {
    #[derive(Debug, PartialEq, Eq)]
    struct State {
        int32 activeTick;
        uint8 status;
        uint128 binCounter;
        uint64 protocolFeeRatio;
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

    #[derive(Debug, PartialEq, Eq)]
    struct BinDelta {
        uint128 deltaA;
        uint128 deltaB;
        uint256 deltaLpBalance;
        uint128 binId;
        uint8 kind;
        int32 lowerTick;
        bool isActive;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TwaState {
        int96 twa;
        int96 value;
        uint64 lastTimestamp;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct AddLiquidityParams {
        uint8 kind;
        int32 pos;
        bool isDelta;
        uint128 deltaA;
        uint128 deltaB;
    }
    #[derive(Debug, PartialEq, Eq)]
    struct RemoveLiquidityParams {
        uint128 binId;
        uint128 amount;
    }

    #[sol(abi=true,rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IMaverickPool {
        event Swap(address sender, address recipient, bool tokenAIn, bool exactOutput, uint256 amountIn, uint256 amountOut, int32 activeTick);
        event AddLiquidity(address indexed sender, uint256 indexed tokenId, BinDelta[] binDeltas);
        event MigrateBinsUpStack(address indexed sender, uint128 binId, uint32 maxRecursion);
        event TransferLiquidity(uint256 fromTokenId, uint256 toTokenId, RemoveLiquidityParams[] params);
        event RemoveLiquidity(address indexed sender, address indexed recipient, uint256 indexed tokenId, BinDelta[] binDeltas);
        event BinMerged(uint128 indexed binId, uint128 reserveA, uint128 reserveB, uint128 mergeId);
        event BinMoved(uint128 indexed binId, int128 previousTick, int128 newTick);
        event ProtocolFeeCollected(uint256 protocolFee, bool isTokenA);
        event SetProtocolFeeRatio(uint256 protocolFee);


        function fee() external view returns (uint256);
        function lookback() external view returns (int256);
        function tickSpacing() external view returns (uint256);
        function tokenA() external view returns (address);
        function tokenB() external view returns (address);
        function factory() external view returns (address);

        function binMap(int32 tick) external view returns (uint256);
        function binPositions(int32 tick, uint256 kind) external view returns (uint128);
        function binBalanceA() external view returns (uint128);
        function binBalanceB() external view returns (uint128);
        function getTwa() external view returns (TwaState memory);
        function getCurrentTwa() external view returns (int256);
        function getState() external view returns (State memory);
        function getBin(uint128 binId) external view returns (BinState memory bin);

        function balanceOf(uint256 tokenId, uint128 binId) external view returns (uint256 lpToken);

        function tokenAScale() external view returns (uint256);

        function tokenBScale() external view returns (uint256);
        function swap(
            address recipient,
            uint256 amount,
            bool tokenAIn,
            bool exactOutput,
            uint256 sqrtPriceLimit,
            bytes calldata data
        ) external returns (uint256 amountIn, uint256 amountOut);
    }
}
