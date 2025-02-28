use alloy::sol;

sol! {
    #[sol(abi=true,rpc)]
    #[derive(Debug, PartialEq, Eq)]
    #[allow(clippy::too_many_arguments)]
    interface IMaverickV2Factory {
        error FactoryInvalidProtocolFeeRatio(uint8 protocolFeeRatioD3);
        error FactoryInvalidLendingFeeRate(uint256 protocolLendingFeeRateD18);
        error FactoryProtocolFeeOnRenounce(uint8 protocolFeeRatioD3);
        error FactorAlreadyInitialized();
        error FactorNotInitialized();
        error FactoryInvalidTokenOrder(address _tokenA, address _tokenB);
        error FactoryInvalidFee();
        error FactoryInvalidKinds(uint8 kinds);
        error FactoryInvalidTickSpacing(uint256 tickSpacing);
        error FactoryInvalidLookback(uint256 lookback);
        error FactoryInvalidTokenDecimals(uint8 decimalsA, uint8 decimalsB);
        error FactoryPoolAlreadyExists(
            uint256 feeAIn,
            uint256 feeBIn,
            uint256 tickSpacing,
            uint256 lookback,
            address tokenA,
            address tokenB,
            uint8 kinds,
            address accessor
        );
        error FactoryAccessorMustBeNonZero();

        event PoolCreated(
            address poolAddress,
            uint8 protocolFeeRatio,
            uint256 feeAIn,
            uint256 feeBIn,
            uint256 tickSpacing,
            uint256 lookback,
            int32 activeTick,
            address tokenA,
            address tokenB,
            uint8 kinds,
            address accessor
        );
        event SetFactoryProtocolFeeRatio(uint8 protocolFeeRatioD3);
        event SetFactoryProtocolLendingFeeRate(uint256 lendingFeeRateD18);
        event SetFactoryProtocolFeeReceiver(address receiver);

        struct DeployParameters {
            uint64 feeAIn;
            uint64 feeBIn;
            uint32 lookback;
            int32 activeTick;
            uint64 tokenAScale;
            uint64 tokenBScale;
            // slot
            address tokenA;
            // slot
            address tokenB;
            // slot
            uint16 tickSpacing;
            uint8 options;
            address accessor;
        }

        /**
         * @notice Called by deployer library to initialize a pool.
         */
        function deployParameters()
            external
            view
            returns (
                uint64 feeAIn,
                uint64 feeBIn,
                uint32 lookback,
                int32 activeTick,
                uint64 tokenAScale,
                uint64 tokenBScale,
                // slot
                address tokenA,
                // slot
                address tokenB,
                // slot
                uint16 tickSpacing,
                uint8 options,
                address accessor
            );

        /**
         * @notice Create a new MaverickV2Pool with symmetric swap fees.
         * @param fee Fraction of the pool swap amount that is retained as an LP in
         * D18 scale.
         * @param tickSpacing Tick spacing of pool where 1.0001^tickSpacing is the
         * bin width.
         * @param lookback Pool lookback in seconds.
         * @param tokenA Address of tokenA.
         * @param tokenB Address of tokenB.
         * @param activeTick Tick position that contains the active bins.
         * @param kinds 1-15 number to represent the active kinds
         * 0b0001 = static;
         * 0b0010 = right;
         * 0b0100 = left;
         * 0b1000 = both.
         * E.g. a pool with all 4 modes will have kinds = b1111 = 15
         */
        function create(
            uint64 fee,
            uint16 tickSpacing,
            uint32 lookback,
            address tokenA,
            address tokenB,
            int32 activeTick,
            uint8 kinds
        ) external returns (address);

        /**
         * @notice Create a new MaverickV2Pool.
         * @param feeAIn Fraction of the pool swap amount for tokenA-input swaps
         * that is retained as an LP in D18 scale.
         * @param feeBIn Fraction of the pool swap amount for tokenB-input swaps
         * that is retained as an LP in D18 scale.
         * @param tickSpacing Tick spacing of pool where 1.0001^tickSpacing is the
         * bin width.
         * @param lookback Pool lookback in seconds.
         * @param tokenA Address of tokenA.
         * @param tokenB Address of tokenB.
         * @param activeTick Tick position that contains the active bins.
         * @param kinds 1-15 number to represent the active kinds
         * 0b0001 = static;
         * 0b0010 = right;
         * 0b0100 = left;
         * 0b1000 = both.
         * e.g. a pool with all 4 modes will have kinds = b1111 = 15
         */
        function create(
            uint64 feeAIn,
            uint64 feeBIn,
            uint16 tickSpacing,
            uint32 lookback,
            address tokenA,
            address tokenB,
            int32 activeTick,
            uint8 kinds
        ) external returns (address);

        /**
         * @notice Create a new MaverickV2PoolPermissioned with symmetric swap fees
         * with all functions permissioned.  Set fee to zero to make the pool fee settable by the accessor.
         * @param fee Fraction of the pool swap amount that is retained as an LP in
         * D18 scale.
         * @param tickSpacing Tick spacing of pool where 1.0001^tickSpacing is the
         * bin width.
         * @param lookback Pool lookback in seconds.
         * @param tokenA Address of tokenA.
         * @param tokenB Address of tokenB.
         * @param activeTick Tick position that contains the active bins.
         * @param kinds 1-15 number to represent the active kinds
         * 0b0001 = static;
         * 0b0010 = right;
         * 0b0100 = left;
         * 0b1000 = both.
         * E.g. a pool with all 4 modes will have kinds = b1111 = 15
         * @param accessor Only address that can access the pool's public write functions.
         */
        function createPermissioned(
            uint64 fee,
            uint16 tickSpacing,
            uint32 lookback,
            address tokenA,
            address tokenB,
            int32 activeTick,
            uint8 kinds,
            address accessor
        ) external returns (address);

        /**
         * @notice Create a new MaverickV2PoolPermissioned with all functions
         * permissioned. Set fees to zero to make the pool fee settable by the
         * accessor.
         * @param feeAIn Fraction of the pool swap amount for tokenA-input swaps
         * that is retained as an LP in D18 scale.
         * @param feeBIn Fraction of the pool swap amount for tokenB-input swaps
         * that is retained as an LP in D18 scale.
         * @param tickSpacing Tick spacing of pool where 1.0001^tickSpacing is the
         * bin width.
         * @param lookback Pool lookback in seconds.
         * @param tokenA Address of tokenA.
         * @param tokenB Address of tokenB.
         * @param activeTick Tick position that contains the active bins.
         * @param kinds 1-15 number to represent the active kinds
         * 0b0001 = static;
         * 0b0010 = right;
         * 0b0100 = left;
         * 0b1000 = both.
         * E.g. a pool with all 4 modes will have kinds = b1111 = 15
         * @param accessor only address that can access the pool's public write functions.
         */
        function createPermissioned(
            uint64 feeAIn,
            uint64 feeBIn,
            uint16 tickSpacing,
            uint32 lookback,
            address tokenA,
            address tokenB,
            int32 activeTick,
            uint8 kinds,
            address accessor
        ) external returns (address);

        /**
         * @notice Create a new MaverickV2PoolPermissioned with the option to make
         * a subset of function permissionless. Set fee to zero to make the pool
         * fee settable by the accessor.
         * @param feeAIn Fraction of the pool swap amount for tokenA-input swaps
         * that is retained as an LP in D18 scale.
         * @param feeBIn Fraction of the pool swap amount for tokenB-input swaps
         * that is retained as an LP in D18 scale.
         * @param tickSpacing Tick spacing of pool where 1.0001^tickSpacing is the
         * bin width.
         * @param lookback Pool lookback in seconds.
         * @param tokenA Address of tokenA.
         * @param tokenB Address of tokenB.
         * @param activeTick Tick position that contains the active bins.
         * @param kinds 1-15 number to represent the active kinds
         * 0b0001 = static;
         * 0b0010 = right;
         * 0b0100 = left;
         * 0b1000 = both.
         * E.g. a pool with all 4 modes will have kinds = b1111 = 15
         * @param accessor only address that can access the pool's public permissioned write functions.
         * @param  permissionedLiquidity If true, then only accessor can call
         * pool's liquidity management functions: `flashLoan`,
         * `migrateBinsUpstack`, `addLiquidity`, `removeLiquidity`.
         * @param  permissionedSwap If true, then only accessor can call
         * pool's swap function.
         */
        function createPermissioned(
            uint64 feeAIn,
            uint64 feeBIn,
            uint16 tickSpacing,
            uint32 lookback,
            address tokenA,
            address tokenB,
            int32 activeTick,
            uint8 kinds,
            address accessor,
            bool permissionedLiquidity,
            bool permissionedSwap
        ) external returns (address pool);

        /**
         * @notice Update the protocol fee ratio for a pool. Can be called
         * permissionlessly allowing any user to sync the pool protocol fee value
         * with the factory protocol fee value.
         * @param pool The pool for which to update.
         */
        function updateProtocolFeeRatioForPool(address pool) external;

        /**
         * @notice Update the protocol lending fee rate for a pool. Can be called
         * permissionlessly allowing any user to sync the pool protocol lending fee
         * rate value with the factory value.
         * @param pool The pool for which to update.
         */
        function updateProtocolLendingFeeRateForPool(address pool) external;

        /**
         * @notice Claim protocol fee for a pool and transfer it to the protocolFeeReceiver.
         * @param pool The pool from which to claim the protocol fee.
         * @param isTokenA A boolean indicating whether tokenA (true) or tokenB
         * (false) is being collected.
         */
        function claimProtocolFeeForPool(address pool, bool isTokenA) external;

        /**
         * @notice Claim protocol fee for a pool and transfer it to the protocolFeeReceiver.
         * @param pool The pool from which to claim the protocol fee.
         */
        function claimProtocolFeeForPool(address pool) external;

        /**
         * @notice Bool indicating whether the pool was deployed from this factory.
         */
        function isFactoryPool(address pool) external view returns (bool);

        /**
         * @notice Address that receives the protocol fee when users call
         * `claimProtocolFeeForPool`.
         */
        function protocolFeeReceiver() external view returns (address);

        /**
         * @notice Lookup a pool for given parameters.
         *
         * @dev  options bit map of kinds and function permissions
         * 0b000001 = static;
         * 0b000010 = right;
         * 0b000100 = left;
         * 0b001000 = both;
         * 0b010000 = liquidity functions are permissioned
         * 0b100000 = swap function is permissioned
         */
        function lookupPermissioned(
            uint256 feeAIn,
            uint256 feeBIn,
            uint256 tickSpacing,
            uint256 lookback,
            address tokenA,
            address tokenB,
            uint8 options,
            address accessor
        ) external view returns (address);

        /**
         * @notice Lookup a pool for given parameters.
         */
        function lookupPermissioned(
            address _tokenA,
            address _tokenB,
            address accessor,
            uint256 startIndex,
            uint256 endIndex
        ) external view returns (address[] memory pools);

        /**
         * @notice Lookup a pool for given parameters.
         */
        function lookupPermissioned(
            uint256 startIndex,
            uint256 endIndex
        ) external view returns (address[] memory pools);

        /**
         * @notice Lookup a pool for given parameters.
         */
        function lookup(
            uint256 feeAIn,
            uint256 feeBIn,
            uint256 tickSpacing,
            uint256 lookback,
            address tokenA,
            address tokenB,
            uint8 kinds
        ) external view returns (address);

        /**
         * @notice Lookup a pool for given parameters.
         */
        function lookup(
            address _tokenA,
            address _tokenB,
            uint256 startIndex,
            uint256 endIndex
        ) external view returns (address[] memory pools);

        /**
         * @notice Lookup a pool for given parameters.
         */
        function lookup(uint256 startIndex, uint256 endIndex) external view returns (address[] memory pools);

        /**
         * @notice Count of permissionless pools.
         */
        function poolCount() external view returns (uint256 _poolCount);

        /**
         * @notice Count of permissioned pools.
         */
        function poolPermissionedCount() external view returns (uint256 _poolCount);

        /**
         * @notice Count of pools for a given accessor and token pair.  For
         * permissionless pools, pass `accessor = address(0)`.
         */
        function poolByTokenCount(
            address _tokenA,
            address _tokenB,
            address accessor
        ) external view returns (uint256 _poolCount);

        /**
         * @notice Get the current factory owner.
         */
        function owner() external view returns (address);

        /**
         * @notice Proportion of protocol fee to collect on each swap.  Value is in
         * 3-decimal format with a maximum value of 0.25e3.
         */
        function protocolFeeRatioD3() external view returns (uint8);

        /**
         * @notice Fee rate charged by the protocol for flashloans.  Value is in
         * 18-decimal format with a maximum value of 0.02e18.
         */
        function protocolLendingFeeRateD18() external view returns (uint256);
    }
}
