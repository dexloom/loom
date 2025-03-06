use alloy::sol;

sol! {
    /**
     * @notice Tick state parameters.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct TickState {
        uint128 reserveA;
        uint128 reserveB;
        uint128 totalSupply;
        uint32[4] binIdsByTick;
    }

    /**
     * @notice Tick data parameters.
     * @param currentReserveA Current reserve of token A.
     * @param currentReserveB Current reserve of token B.
     * @param currentLiquidity Current liquidity amount.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct TickData {
        uint256 currentReserveA;
        uint256 currentReserveB;
        uint256 currentLiquidity;
    }

    /**
     * @notice Bin state parameters.
     * @param mergeBinBalance LP token balance that this bin possesses of the merge bin.
     * @param mergeId Bin ID of the bin that this bin has merged into.
     * @param totalSupply Total amount of LP tokens in this bin.
     * @param kind One of the 4 kinds (0=static, 1=right, 2=left, 3=both).
     * @param tick The lower price tick of the bin in its current state.
     * @param tickBalance Balance of the tick.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct BinState {
        uint128 mergeBinBalance;
        uint128 tickBalance;
        uint128 totalSupply;
        uint8 kind;
        int32 tick;
        uint32 mergeId;
    }

    /**
     * @notice Parameters for swap.
     * @param amount Amount of the token that is either the input if exactOutput is false
     * or the output if exactOutput is true.
     * @param tokenAIn Boolean indicating whether tokenA is the input.
     * @param exactOutput Boolean indicating whether the amount specified is
     * the exact output amount (true).
     * @param tickLimit The furthest tick a swap will execute in. If no limit
     * is desired, value should be set to type(int32).max for a tokenAIn swap
     * and type(int32).min for a swap where tokenB is the input.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct SwapParams {
        uint256 amount;
        bool tokenAIn;
        bool exactOutput;
        int32 tickLimit;
    }

    /**
     * @notice Parameters associated with adding liquidity.
     * @param kind One of the 4 kinds (0=static, 1=right, 2=left, 3=both).
     * @param ticks Array of ticks to add liquidity to.
     * @param amounts Array of bin LP amounts to add.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct AddLiquidityParams {
        uint8 kind;
        int32[] ticks;
        uint128[] amounts;
    }

    /**
     * @notice Parameters for each bin that will have liquidity removed.
     * @param binIds Index array of the bins losing liquidity.
     * @param amounts Array of bin LP amounts to remove.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct RemoveLiquidityParams {
        uint32[] binIds;
        uint128[] amounts;
    }

    /**
     * @notice State of the pool.
     * @param reserveA Pool tokenA balanceOf at end of last operation
     * @param reserveB Pool tokenB balanceOf at end of last operation
     * @param lastTwaD8 Value of log time weighted average price at last block.
     * Value is 8-decimal scale and is in the fractional tick domain.  E.g. a
     * value of 12.3e8 indicates the TWAP was 3/10ths of the way into the 12th
     * tick.
     * @param lastLogPriceD8 Value of log price at last block. Value is
     * 8-decimal scale and is in the fractional tick domain.  E.g. a value of
     * 12.3e8 indicates the price was 3/10ths of the way into the 12th tick.
     * @param lastTimestamp Last block.timestamp value in seconds for latest
     * swap transaction.
     * @param activeTick Current tick position that contains the active bins.
     * @param isLocked Pool isLocked, E.g., locked or unlocked; isLocked values
     * defined in Pool.sol.
     * @param binCounter Index of the last bin created.
     * @param protocolFeeRatioD3 Ratio of the swap fee that is kept for the
     * protocol.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct State {
        uint128 reserveA;
        uint128 reserveB;
        int64 lastTwaD8;
        int64 lastLogPriceD8;
        uint40 lastTimestamp;
        int32 activeTick;
        bool isLocked;
        uint32 binCounter;
        uint8 protocolFeeRatioD3;
    }

    /**
     * @notice Internal data used for data passing between Pool and Bin code.
     */
    #[derive(Debug, PartialEq, Eq)]
    struct BinDelta {
        uint128 deltaA;
        uint128 deltaB;
    }

    #[sol(abi=true,rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IMaverickV2Pool {
        error PoolZeroLiquidityAdded();
        error PoolMinimumLiquidityNotMet();
        error PoolLocked();
        error PoolInvalidFee();
        error PoolTicksNotSorted(uint256 index, int256 previousTick, int256 tick);
        error PoolTicksAmountsLengthMismatch(uint256 ticksLength, uint256 amountsLength);
        error PoolBinIdsAmountsLengthMismatch(uint256 binIdsLength, uint256 amountsLength);
        error PoolKindNotSupported(uint256 kinds, uint256 kind);
        error PoolInsufficientBalance(uint256 deltaLpAmount, uint256 accountBalance);
        error PoolReservesExceedMaximum(uint256 amount);
        error PoolValueExceedsBits(uint256 amount, uint256 bits);
        error PoolTickMaxExceeded(uint256 tick);
        error PoolMigrateBinFirst();
        error PoolCurrentTickBeyondSwapLimit(int32 startingTick);
        error PoolSenderNotAccessor(address sender_, address accessor);
        error PoolSenderNotFactory(address sender_, address accessor);
        error PoolFunctionNotImplemented();
        error PoolTokenNotSolvent(uint256 internalReserve, uint256 tokenBalance, address token);

        event PoolSwap(address sender, address recipient, SwapParams params, uint256 amountIn, uint256 amountOut);

        event PoolAddLiquidity(
            address sender,
            address recipient,
            uint256 subaccount,
            AddLiquidityParams params,
            uint256 tokenAAmount,
            uint256 tokenBAmount,
            uint32[] binIds
        );

        event PoolMigrateBinsUpStack(address sender, uint32 binId, uint32 maxRecursion);

        event PoolRemoveLiquidity(
            address sender,
            address recipient,
            uint256 subaccount,
            RemoveLiquidityParams params,
            uint256 tokenAOut,
            uint256 tokenBOut
        );

        event PoolSetVariableFee(uint256 newFeeAIn, uint256 newFeeBIn);



        /**
         * @notice 1-15 number to represent the active kinds.
         * @notice 0b0001 = static;
         * @notice 0b0010 = right;
         * @notice 0b0100 = left;
         * @notice 0b1000 = both;
         *
         * E.g. a pool with all 4 modes will have kinds = b1111 = 15
         */
        function kinds() external view returns (uint8 _kinds);

        /**
         * @notice Returns whether a pool has permissioned functions. If true, the
         * `accessor()` of the pool can set the pool fees.  Other functions in the
         * pool may also be permissioned; whether or not they are can be determined
         * through calls to `permissionedLiquidity()` and `permissionedSwap()`.
         */
        function permissionedPool() external view returns (bool _permissionedPool);

        /**
         * @notice Returns whether a pool has permissioned liquidity management
         * functions. If true, the pool is incompatible with permissioned pool
         * liquidity management infrastructure.
         */
        function permissionedLiquidity() external view returns (bool _permissionedLiquidity);

        /**
         * @notice Returns whether a pool has a permissioned swap functions. If
         * true, the pool is incompatible with permissioned pool swap router
         * infrastructure.
         */
        function permissionedSwap() external view returns (bool _permissionedSwap);

        /**
         * @notice Pool swap fee for the given direction (A-in or B-in swap) in
         * 18-decimal format. E.g. 0.01e18 is a 1% swap fee.
         */
        function fee(bool tokenAIn) external view returns (uint256);

        /**
         * @notice TickSpacing of pool where 1.0001^tickSpacing is the bin width.
         */
        function tickSpacing() external view returns (uint256);

        /**
         * @notice Lookback period of pool in seconds.
         */
        function lookback() external view returns (uint256);

        /**
         * @notice Address of Pool accessor.  This is Zero address for
         * permissionless pools.
         */
        function accessor() external view returns (address);

        /**
         * @notice Pool tokenA.  Address of tokenA is such that tokenA < tokenB.
         */
        function tokenA() external view returns (address);

        /**
         * @notice Pool tokenB.
         */
        function tokenB() external view returns (address);

        /**
         * @notice Deploying factory of the pool and also contract that has ability
         * to set and collect protocol fees for the pool.
         */
        function factory() external view returns (address);

        /**
         * @notice Most significant bit of scale value is a flag to indicate whether
         * tokenA has more or less than 18 decimals.  Scale is used in conjuction
         * with Math.toScale/Math.fromScale functions to convert from token amounts
         * to D18 scale internal pool accounting.
         */
        function tokenAScale() external view returns (uint256);

        /**
         * @notice Most significant bit of scale value is a flag to indicate whether
         * tokenA has more or less than 18 decimals.  Scale is used in conjuction
         * with Math.toScale/Math.fromScale functions to convert from token amounts
         * to D18 scale internal pool accounting.
         */
        function tokenBScale() external view returns (uint256);

        /**
         * @notice ID of bin at input tick position and kind.
         */
        function binIdByTickKind(int32 tick, uint256 kind) external view returns (uint32);

        /**
         * @notice Accumulated tokenA protocol fee.
         */
        function protocolFeeA() external view returns (uint128);

        /**
         * @notice Accumulated tokenB protocol fee.
         */
        function protocolFeeB() external view returns (uint128);

        /**
         * @notice Lending fee rate on flash loans.
         */
        function lendingFeeRateD18() external view returns (uint256);

        /**
         * @notice External function to get the current time-weighted average price.
         */
        function getCurrentTwa() external view returns (int256);

        /**
         * @notice External function to get the state of the pool.
         */
        function getState() external view returns (State memory);

        /**
         * @notice Return state of Bin at input binId.
         */
        function getBin(uint32 binId) external view returns (BinState memory bin);

        /**
         * @notice Return state of Tick at input tick position.
         */
        function getTick(int32 tick) external view returns (TickState memory tickState);

        /**
         * @notice Retrieves the balance of a user within a bin.
         * @param user The user's address.
         * @param subaccount The subaccount for the user.
         * @param binId The ID of the bin.
         */
        function balanceOf(address user, uint256 subaccount, uint32 binId) external view returns (uint128 lpToken);

        /**
         * @notice Add liquidity to a pool. This function allows users to deposit
         * tokens into a liquidity pool.
         * @dev This function will call `maverickV2AddLiquidityCallback` on the
         * calling contract to collect the tokenA/tokenB payment.
         * @param recipient The account that will receive credit for the added liquidity.
         * @param subaccount The account that will receive credit for the added liquidity.
         * @param params Parameters containing the details for adding liquidity,
         * such as token types and amounts.
         * @param data Bytes information that gets passed to the callback.
         * @return tokenAAmount The amount of token A added to the pool.
         * @return tokenBAmount The amount of token B added to the pool.
         * @return binIds An array of bin IDs where the liquidity is stored.
         */
        function addLiquidity(
            address recipient,
            uint256 subaccount,
            AddLiquidityParams calldata params,
            bytes calldata data
        ) external returns (uint256 tokenAAmount, uint256 tokenBAmount, uint32[] memory binIds);

        /**
         * @notice Removes liquidity from the pool.
         * @dev Liquidy can only be removed from a bin that is either unmerged or
         * has a mergeId of an unmerged bin.  If a bin is merged more than one
         * level deep, it must be migrated up the merge stack to the root bin
         * before liquidity removal.
         * @param recipient The address to receive the tokens.
         * @param subaccount The subaccount for the recipient.
         * @param params The parameters for removing liquidity.
         * @return tokenAOut The amount of token A received.
         * @return tokenBOut The amount of token B received.
         */
        function removeLiquidity(
            address recipient,
            uint256 subaccount,
            RemoveLiquidityParams calldata params
        ) external returns (uint256 tokenAOut, uint256 tokenBOut);

        /**
         * @notice Migrate bins up the linked list of merged bins so that its
         * mergeId is the currrent active bin.
         * @dev Liquidy can only be removed from a bin that is either unmerged or
         * has a mergeId of an unmerged bin.  If a bin is merged more than one
         * level deep, it must be migrated up the merge stack to the root bin
         * before liquidity removal.
         * @param binId The ID of the bin to migrate.
         * @param maxRecursion The maximum recursion depth for the migration.
         */
        function migrateBinUpStack(uint32 binId, uint32 maxRecursion) external;

        /**
         * @notice Swap tokenA/tokenB assets in the pool.  The swap user has two
         * options for funding their swap.
         * - The user can push the input token amount to the pool before calling
         * the swap function. In order to avoid having the pool call the callback,
         * the user should pass a zero-length `data` bytes object with the swap
         * call.
         * - The user can send the input token amount to the pool when the pool
         * calls the `maverickV2SwapCallback` function on the calling contract.
         * That callback has input parameters that specify the token address of the
         * input token, the input and output amounts, and the bytes data sent to
         * the swap function.
         * @dev  If the users elects to do a callback-based swap, the output
         * assets will be sent before the callback is called, allowing the user to
         * execute flash swaps.  However, the pool does have reentrancy protection,
         * so a swapper will not be able to interact with the same pool again
         * while they are in the callback function.
         * @param recipient The address to receive the output tokens.
         * @param params Parameters containing the details of the swap
         * @param data Bytes information that gets passed to the callback.
         */
        function swap(
            address recipient,
            SwapParams memory params,
            bytes calldata data
        ) external returns (uint256 amountIn, uint256 amountOut);

        /**
         * @notice Loan tokenA/tokenB assets from the pool to recipient. The fee
         * rate of a loan is determined by `lendingFeeRateD18`, which is set at the
         * protocol level by the factory.  This function calls
         * `maverickV2FlashLoanCallback` on the calling contract.  At the end of
         * the callback, the caller must pay back the loan with fee (if there is a
         * fee).
         * @param recipient The address to receive the loaned tokens.
         * @param amountB Loan amount of tokenA sent to recipient.
         * @param amountB Loan amount of tokenB sent to recipient.
         * @param data Bytes information that gets passed to the callback.
         */
        function flashLoan(
            address recipient,
            uint256 amountA,
            uint256 amountB,
            bytes calldata data
        ) external returns (uint128 lendingFeeA, uint128 lendingFeeB);

        /**
         * @notice Sets fee for permissioned pools.  May only be called by the
         * accessor.
         */
        function setFee(uint256 newFeeAIn, uint256 newFeeBIn) external;
    }


}
