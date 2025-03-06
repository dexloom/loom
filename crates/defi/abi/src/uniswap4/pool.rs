use alloy::sol;

sol! {

    #[derive(Debug, PartialEq, Eq)]
    type BalanceDelta is int256;

    #[derive(Debug, PartialEq, Eq)]
    type Currency is address;

    #[derive(Debug, PartialEq, Eq)]
    type PoolId is bytes32;

    #[derive(Debug, PartialEq, Eq)]
    type Hooks is address;

    #[derive(Debug, PartialEq, Eq)]
    type BeforeSwapDelta is int256;



    #[derive(Debug, PartialEq, Eq)]
    struct PoolKey {
        /// @notice The lower currency of the pool, sorted numerically
        Currency currency0;
        /// @notice The higher currency of the pool, sorted numerically
        Currency currency1;
        /// @notice The pool swap fee, capped at 1_000_000. The upper 4 bits determine if the hook sets any fees.
        uint24 fee;
        /// @notice Ticks that involve positions must be a multiple of tick spacing
        int24 tickSpacing;
        /// @notice The hooks of the pool
        Hooks hooks;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct PathKey {
        Currency intermediateCurrency;
        uint24 fee;
        int24 tickSpacing;
        IHooks hooks;
        bytes hookData;
    }


    #[derive(Debug, PartialEq, Eq)]
    struct IPoolManagerModifyLiquidityParams {
            // the lower and upper tick of the position
            int24 tickLower;
            int24 tickUpper;
            // how to modify the liquidity
            int256 liquidityDelta;
        }

    #[derive(Debug, PartialEq, Eq)]
    struct IPoolManagerSwapParams {
            bool zeroForOne;
            int256 amountSpecified;
            uint160 sqrtPriceLimitX96;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct PositionInfo {
            // the amount of liquidity owned by this position
            uint128 liquidity;
            // fee growth per unit of liquidity as of the last update to liquidity or fees owed
            uint256 feeGrowthInside0LastX128;
            uint256 feeGrowthInside1LastX128;
    }

    #[derive(Debug, PartialEq, Eq)]
    struct PoolTickInfo {
        // the total position liquidity that references this tick
            uint128 liquidityGross;
            // amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left),
            int128 liquidityNet;
            // fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
            // only has relative meaning, not absolute — the value depends on when the tick is initialized
            uint256 feeGrowthOutside0X128;
            uint256 feeGrowthOutside1X128;
    }

    #[derive(Debug, PartialEq, Eq)]
    library Position {
    /// @notice Cannot update a position with no liquidity
        error CannotUpdateEmptyPosition();

        // info stored for each user's position
        struct Info {
            // the amount of liquidity owned by this position
            uint128 liquidity;
            // fee growth per unit of liquidity as of the last update to liquidity or fees owed
            uint256 feeGrowthInside0LastX128;
            uint256 feeGrowthInside1LastX128;
        }
    }


    #[derive(Debug, PartialEq, Eq)]
    interface IUniswapV4PositionManager {
            function mintPosition(PoolKey poolKey,
                int24 tickLower,
                int24 timckUpper,
                uint256 liquidity,
                uint128 amount0Max,
                uint128 amount1Max,
                address owner,
                bytes hookData);
    }

    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    interface IUniswapV4PoolManagerEvents {
        /// @notice Emitted when a new pool is initialized
        /// @param id The abi encoded hash of the pool key struct for the new pool
        /// @param currency0 The first currency of the pool by address sort order
        /// @param currency1 The second currency of the pool by address sort order
        /// @param fee The fee collected upon every swap in the pool, denominated in hundredths of a bip
        /// @param tickSpacing The minimum number of ticks between initialized ticks
        /// @param hooks The hooks contract address for the pool, or address(0) if none
        /// @param sqrtPriceX96 The price of the pool on initialization
        /// @param tick The initial tick of the pool corresponding to the initialized price
            event Initialize(
                PoolId indexed id,
                Currency indexed currency0,
                Currency indexed currency1,
                uint24 fee,
                int24 tickSpacing,
                Hooks hooks,
                uint160 sqrtPriceX96,
                int24 tick
            );

            /// @notice Emitted when a liquidity position is modified
            /// @param id The abi encoded hash of the pool key struct for the pool that was modified
            /// @param sender The address that modified the pool
            /// @param tickLower The lower tick of the position
            /// @param tickUpper The upper tick of the position
            /// @param liquidityDelta The amount of liquidity that was added or removed
            /// @param salt The extra data to make positions unique
            event ModifyLiquidity(
                PoolId indexed id, address indexed sender, int24 tickLower, int24 tickUpper, int256 liquidityDelta, bytes32 salt
            );

            /// @notice Emitted for swaps between currency0 and currency1
            /// @param id The abi encoded hash of the pool key struct for the pool that was modified
            /// @param sender The address that initiated the swap call, and that received the callback
            /// @param amount0 The delta of the currency0 balance of the pool
            /// @param amount1 The delta of the currency1 balance of the pool
            /// @param sqrtPriceX96 The sqrt(price) of the pool after the swap, as a Q64.96
            /// @param liquidity The liquidity of the pool after the swap
            /// @param tick The log base 1.0001 of the price of the pool after the swap
            /// @param fee The swap fee in hundredths of a bip
            event Swap(
                PoolId indexed id,
                address indexed sender,
                int128 amount0,
                int128 amount1,
                uint160 sqrtPriceX96,
                uint128 liquidity,
                int24 tick,
                uint24 fee
            );

            /// @notice Emitted for donations
            /// @param id The abi encoded hash of the pool key struct for the pool that was donated to
            /// @param sender The address that initiated the donate call
            /// @param amount0 The amount donated in currency0
            /// @param amount1 The amount donated in currency1
            event Donate(PoolId indexed id, address indexed sender, uint256 amount0, uint256 amount1);
    }

    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    interface IUniswapV4PoolManager {    /// @notice Thrown when a currency is not netted out after the contract is unlocked
            error CurrencyNotSettled();

            /// @notice Thrown when trying to interact with a non-initialized pool
            error PoolNotInitialized();

            /// @notice Thrown when unlock is called, but the contract is already unlocked
            error AlreadyUnlocked();

            /// @notice Thrown when a function is called that requires the contract to be unlocked, but it is not
            error ManagerLocked();

            /// @notice Pools are limited to type(int16).max tickSpacing in #initialize, to prevent overflow
            error TickSpacingTooLarge(int24 tickSpacing);

            /// @notice Pools must have a positive non-zero tickSpacing passed to #initialize
            error TickSpacingTooSmall(int24 tickSpacing);

            /// @notice PoolKey must have currencies where address(currency0) < address(currency1)
            error CurrenciesOutOfOrderOrEqual(address currency0, address currency1);

            /// @notice Thrown when a call to updateDynamicLPFee is made by an address that is not the hook,
            /// or on a pool that does not have a dynamic swap fee.
            error UnauthorizedDynamicLPFeeUpdate();

            /// @notice Thrown when trying to swap amount of 0
            error SwapAmountCannotBeZero();

            ///@notice Thrown when native currency is passed to a non native settlement
            error NonzeroNativeValue();

            /// @notice Thrown when `clear` is called with an amount that is not exactly equal to the open currency delta.
            error MustClearExactPositiveDelta();



            /// @notice All interactions on the contract that account deltas require unlocking. A caller that calls `unlock` must implement
            /// `IUnlockCallback(msg.sender).unlockCallback(data)`, where they interact with the remaining functions on this contract.
            /// @dev The only functions callable without an unlocking are `initialize` and `updateDynamicLPFee`
            /// @param data Any data to pass to the callback, via `IUnlockCallback(msg.sender).unlockCallback(data)`
            /// @return The data returned by the call to `IUnlockCallback(msg.sender).unlockCallback(data)`
            function unlock(bytes calldata data) external returns (bytes memory);

            /// @notice Initialize the state for a given pool ID
            /// @dev A swap fee totaling MAX_SWAP_FEE (100%) makes exact output swaps impossible since the input is entirely consumed by the fee
            /// @param key The pool key for the pool to initialize
            /// @param sqrtPriceX96 The initial square root price
            /// @return tick The initial tick of the pool
            function initialize(PoolKey memory key, uint160 sqrtPriceX96) external returns (int24 tick);

            struct ModifyLiquidityParams {
                // the lower and upper tick of the position
                int24 tickLower;
                int24 tickUpper;
                // how to modify the liquidity
                int256 liquidityDelta;
                // a value to set if you want unique liquidity positions at the same range
                bytes32 salt;
            }

            /// @notice Modify the liquidity for the given pool
            /// @dev Poke by calling with a zero liquidityDelta
            /// @param key The pool to modify liquidity in
            /// @param params The parameters for modifying the liquidity
            /// @param hookData The data to pass through to the add/removeLiquidity hooks
            /// @return callerDelta The balance delta of the caller of modifyLiquidity. This is the total of both principal, fee deltas, and hook deltas if applicable
            /// @return feesAccrued The balance delta of the fees generated in the liquidity range. Returned for informational purposes
            function modifyLiquidity(PoolKey memory key, ModifyLiquidityParams memory params, bytes calldata hookData)
                external
                returns (BalanceDelta callerDelta, BalanceDelta feesAccrued);

            struct SwapParams {
                /// Whether to swap token0 for token1 or vice versa
                bool zeroForOne;
                /// The desired input amount if negative (exactIn), or the desired output amount if positive (exactOut)
                int256 amountSpecified;
                /// The sqrt price at which, if reached, the swap will stop executing
                uint160 sqrtPriceLimitX96;
            }

            /// @notice Swap against the given pool
            /// @param key The pool to swap in
            /// @param params The parameters for swapping
            /// @param hookData The data to pass through to the swap hooks
            /// @return swapDelta The balance delta of the address swapping
            /// @dev Swapping on low liquidity pools may cause unexpected swap amounts when liquidity available is less than amountSpecified.
            /// Additionally note that if interacting with hooks that have the BEFORE_SWAP_RETURNS_DELTA_FLAG or AFTER_SWAP_RETURNS_DELTA_FLAG
            /// the hook may alter the swap input/output. Integrators should perform checks on the returned swapDelta.
            function swap(PoolKey memory key, SwapParams memory params, bytes calldata hookData)
                external
                returns (BalanceDelta swapDelta);

            /// @notice Donate the given currency amounts to the in-range liquidity providers of a pool
            /// @dev Calls to donate can be frontrun adding just-in-time liquidity, with the aim of receiving a portion donated funds.
            /// Donors should keep this in mind when designing donation mechanisms.
            /// @dev This function donates to in-range LPs at slot0.tick. In certain edge-cases of the swap algorithm, the `sqrtPrice` of
            /// a pool can be at the lower boundary of tick `n`, but the `slot0.tick` of the pool is already `n - 1`. In this case a call to
            /// `donate` would donate to tick `n - 1` (slot0.tick) not tick `n` (getTickAtSqrtPrice(slot0.sqrtPriceX96)).
            /// Read the comments in `Pool.swap()` for more information about this.
            /// @param key The key of the pool to donate to
            /// @param amount0 The amount of currency0 to donate
            /// @param amount1 The amount of currency1 to donate
            /// @param hookData The data to pass through to the donate hooks
            /// @return BalanceDelta The delta of the caller after the donate
            function donate(PoolKey memory key, uint256 amount0, uint256 amount1, bytes calldata hookData)
                external
                returns (BalanceDelta);

            /// @notice Writes the current ERC20 balance of the specified currency to transient storage
            /// This is used to checkpoint balances for the manager and derive deltas for the caller.
            /// @dev This MUST be called before any ERC20 tokens are sent into the contract, but can be skipped
            /// for native tokens because the amount to settle is determined by the sent value.
            /// However, if an ERC20 token has been synced and not settled, and the caller instead wants to settle
            /// native funds, this function can be called with the native currency to then be able to settle the native currency
            function sync(Currency currency) external;

            /// @notice Called by the user to net out some value owed to the user
            /// @dev Will revert if the requested amount is not available, consider using `mint` instead
            /// @dev Can also be used as a mechanism for free flash loans
            /// @param currency The currency to withdraw from the pool manager
            /// @param to The address to withdraw to
            /// @param amount The amount of currency to withdraw
            function take(Currency currency, address to, uint256 amount) external;

            /// @notice Called by the user to pay what is owed
            /// @return paid The amount of currency settled
            function settle() external payable returns (uint256 paid);

            /// @notice Called by the user to pay on behalf of another address
            /// @param recipient The address to credit for the payment
            /// @return paid The amount of currency settled
            function settleFor(address recipient) external payable returns (uint256 paid);

            /// @notice WARNING - Any currency that is cleared, will be non-retrievable, and locked in the contract permanently.
            /// A call to clear will zero out a positive balance WITHOUT a corresponding transfer.
            /// @dev This could be used to clear a balance that is considered dust.
            /// Additionally, the amount must be the exact positive balance. This is to enforce that the caller is aware of the amount being cleared.
            function clear(Currency currency, uint256 amount) external;

            /// @notice Called by the user to move value into ERC6909 balance
            /// @param to The address to mint the tokens to
            /// @param id The currency address to mint to ERC6909s, as a uint256
            /// @param amount The amount of currency to mint
            /// @dev The id is converted to a uint160 to correspond to a currency address
            /// If the upper 12 bytes are not 0, they will be 0-ed out
            function mint(address to, uint256 id, uint256 amount) external;

            /// @notice Called by the user to move value from ERC6909 balance
            /// @param from The address to burn the tokens from
            /// @param id The currency address to burn from ERC6909s, as a uint256
            /// @param amount The amount of currency to burn
            /// @dev The id is converted to a uint160 to correspond to a currency address
            /// If the upper 12 bytes are not 0, they will be 0-ed out
            function burn(address from, uint256 id, uint256 amount) external;

            /// @notice Updates the pools lp fees for the a pool that has enabled dynamic lp fees.
            /// @dev A swap fee totaling MAX_SWAP_FEE (100%) makes exact output swaps impossible since the input is entirely consumed by the fee
            /// @param key The key of the pool to update dynamic LP fees for
            /// @param newDynamicLPFee The new dynamic pool LP fee
            function updateDynamicLPFee(PoolKey memory key, uint24 newDynamicLPFee) external;

    }

    #[derive(Debug, PartialEq, Eq)]
    interface IHooks {
            /// @notice The hook called before the state of a pool is initialized
            /// @param sender The initial msg.sender for the initialize call
            /// @param key The key for the pool being initialized
            /// @param sqrtPriceX96 The sqrt(price) of the pool as a Q64.96
            /// @return bytes4 The function selector for the hook
            function beforeInitialize(address sender, PoolKey calldata key, uint160 sqrtPriceX96) external returns (bytes4);

            /// @notice The hook called after the state of a pool is initialized
            /// @param sender The initial msg.sender for the initialize call
            /// @param key The key for the pool being initialized
            /// @param sqrtPriceX96 The sqrt(price) of the pool as a Q64.96
            /// @param tick The current tick after the state of a pool is initialized
            /// @return bytes4 The function selector for the hook
            function afterInitialize(address sender, PoolKey calldata key, uint160 sqrtPriceX96, int24 tick)
                external
                returns (bytes4);

            /// @notice The hook called before liquidity is added
            /// @param sender The initial msg.sender for the add liquidity call
            /// @param key The key for the pool
            /// @param params The parameters for adding liquidity
            /// @param hookData Arbitrary data handed into the PoolManager by the liquidity provider to be passed on to the hook
            /// @return bytes4 The function selector for the hook
            function beforeAddLiquidity(
                address sender,
                PoolKey calldata key,
                IPoolManagerModifyLiquidityParams calldata params,
                bytes calldata hookData
            ) external returns (bytes4);

            /// @notice The hook called after liquidity is added
            /// @param sender The initial msg.sender for the add liquidity call
            /// @param key The key for the pool
            /// @param params The parameters for adding liquidity
            /// @param delta The caller's balance delta after adding liquidity; the sum of principal delta, fees accrued, and hook delta
            /// @param feesAccrued The fees accrued since the last time fees were collected from this position
            /// @param hookData Arbitrary data handed into the PoolManager by the liquidity provider to be passed on to the hook
            /// @return bytes4 The function selector for the hook
            /// @return BalanceDelta The hook's delta in token0 and token1. Positive: the hook is owed/took currency, negative: the hook owes/sent currency
            function afterAddLiquidity(
                address sender,
                PoolKey calldata key,
                IPoolManagerModifyLiquidityParams calldata params,
                BalanceDelta delta,
                BalanceDelta feesAccrued,
                bytes calldata hookData
            ) external returns (bytes4, BalanceDelta);

            /// @notice The hook called before liquidity is removed
            /// @param sender The initial msg.sender for the remove liquidity call
            /// @param key The key for the pool
            /// @param params The parameters for removing liquidity
            /// @param hookData Arbitrary data handed into the PoolManager by the liquidity provider to be be passed on to the hook
            /// @return bytes4 The function selector for the hook
            function beforeRemoveLiquidity(
                address sender,
                PoolKey calldata key,
                IPoolManagerModifyLiquidityParams calldata params,
                bytes calldata hookData
            ) external returns (bytes4);

            /// @notice The hook called after liquidity is removed
            /// @param sender The initial msg.sender for the remove liquidity call
            /// @param key The key for the pool
            /// @param params The parameters for removing liquidity
            /// @param delta The caller's balance delta after removing liquidity; the sum of principal delta, fees accrued, and hook delta
            /// @param feesAccrued The fees accrued since the last time fees were collected from this position
            /// @param hookData Arbitrary data handed into the PoolManager by the liquidity provider to be be passed on to the hook
            /// @return bytes4 The function selector for the hook
            /// @return BalanceDelta The hook's delta in token0 and token1. Positive: the hook is owed/took currency, negative: the hook owes/sent currency
            function afterRemoveLiquidity(
                address sender,
                PoolKey calldata key,
                IPoolManagerModifyLiquidityParams calldata params,
                BalanceDelta delta,
                BalanceDelta feesAccrued,
                bytes calldata hookData
            ) external returns (bytes4, BalanceDelta);

            /// @notice The hook called before a swap
            /// @param sender The initial msg.sender for the swap call
            /// @param key The key for the pool
            /// @param params The parameters for the swap
            /// @param hookData Arbitrary data handed into the PoolManager by the swapper to be be passed on to the hook
            /// @return bytes4 The function selector for the hook
            /// @return BeforeSwapDelta The hook's delta in specified and unspecified currencies. Positive: the hook is owed/took currency, negative: the hook owes/sent currency
            /// @return uint24 Optionally override the lp fee, only used if three conditions are met: 1. the Pool has a dynamic fee, 2. the value's 2nd highest bit is set (23rd bit, 0x400000), and 3. the value is less than or equal to the maximum fee (1 million)
            function beforeSwap(
                address sender,
                PoolKey calldata key,
                IPoolManagerSwapParams calldata params,
                bytes calldata hookData
            ) external returns (bytes4, BeforeSwapDelta, uint24);

            /// @notice The hook called after a swap
            /// @param sender The initial msg.sender for the swap call
            /// @param key The key for the pool
            /// @param params The parameters for the swap
            /// @param delta The amount owed to the caller (positive) or owed to the pool (negative)
            /// @param hookData Arbitrary data handed into the PoolManager by the swapper to be be passed on to the hook
            /// @return bytes4 The function selector for the hook
            /// @return int128 The hook's delta in unspecified currency. Positive: the hook is owed/took currency, negative: the hook owes/sent currency
            function afterSwap(
                address sender,
                PoolKey calldata key,
                IPoolManagerSwapParams calldata params,
                BalanceDelta delta,
                bytes calldata hookData
            ) external returns (bytes4, int128);

            /// @notice The hook called before donate
            /// @param sender The initial msg.sender for the donate call
            /// @param key The key for the pool
            /// @param amount0 The amount of token0 being donated
            /// @param amount1 The amount of token1 being donated
            /// @param hookData Arbitrary data handed into the PoolManager by the donor to be be passed on to the hook
            /// @return bytes4 The function selector for the hook
            function beforeDonate(
                address sender,
                PoolKey calldata key,
                uint256 amount0,
                uint256 amount1,
                bytes calldata hookData
            ) external returns (bytes4);

            /// @notice The hook called after donate
            /// @param sender The initial msg.sender for the donate call
            /// @param key The key for the pool
            /// @param amount0 The amount of token0 being donated
            /// @param amount1 The amount of token1 being donated
            /// @param hookData Arbitrary data handed into the PoolManager by the donor to be be passed on to the hook
            /// @return bytes4 The function selector for the hook
            function afterDonate(
                address sender,
                PoolKey calldata key,
                uint256 amount0,
                uint256 amount1,
                bytes calldata hookData
            ) external returns (bytes4);

    }

    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    interface IV4Quoter  {

        struct QuoteExactSingleParams {
            PoolKey poolKey;
            bool zeroForOne;
            uint128 exactAmount;
            bytes hookData;
        }

        struct QuoteExactParams {
            Currency exactCurrency;
            PathKey[] path;
            uint128 exactAmount;
        }

        /// @notice Returns the delta amounts for a given exact input swap of a single pool
        /// @param params The params for the quote, encoded as `QuoteExactSingleParams`
        /// poolKey The key for identifying a V4 pool
        /// zeroForOne If the swap is from currency0 to currency1
        /// exactAmount The desired input amount
        /// hookData arbitrary hookData to pass into the associated hooks
        /// @return amountOut The output quote for the exactIn swap
        /// @return gasEstimate Estimated gas units used for the swap
        function quoteExactInputSingle(QuoteExactSingleParams memory params)
            external
            returns (uint256 amountOut, uint256 gasEstimate);

        /// @notice Returns the delta amounts along the swap path for a given exact input swap
        /// @param params the params for the quote, encoded as 'QuoteExactParams'
        /// currencyIn The input currency of the swap
        /// path The path of the swap encoded as PathKeys that contains currency, fee, tickSpacing, and hook info
        /// exactAmount The desired input amount
        /// @return amountOut The output quote for the exactIn swap
        /// @return gasEstimate Estimated gas units used for the swap
        function quoteExactInput(QuoteExactParams memory params)
            external
            returns (uint256 amountOut, uint256 gasEstimate);

        /// @notice Returns the delta amounts for a given exact output swap of a single pool
        /// @param params The params for the quote, encoded as `QuoteExactSingleParams`
        /// poolKey The key for identifying a V4 pool
        /// zeroForOne If the swap is from currency0 to currency1
        /// exactAmount The desired output amount
        /// hookData arbitrary hookData to pass into the associated hooks
        /// @return amountIn The input quote for the exactOut swap
        /// @return gasEstimate Estimated gas units used for the swap
        function quoteExactOutputSingle(QuoteExactSingleParams memory params)
            external
            returns (uint256 amountIn, uint256 gasEstimate);

        /// @notice Returns the delta amounts along the swap path for a given exact output swap
        /// @param params the params for the quote, encoded as 'QuoteExactParams'
        /// currencyOut The output currency of the swap
        /// path The path of the swap encoded as PathKeys that contains currency, fee, tickSpacing, and hook info
        /// exactAmount The desired output amount
        /// @return amountIn The input quote for the exactOut swap
        /// @return gasEstimate Estimated gas units used for the swap
        function quoteExactOutput(QuoteExactParams memory params)
            external
            returns (uint256 amountIn, uint256 gasEstimate);
    }

    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    interface IStateView is IImmutableState {
        /// @notice Get Slot0 of the pool: sqrtPriceX96, tick, protocolFee, lpFee
        /// @dev Corresponds to pools[poolId].slot0
        /// @param poolId The ID of the pool.
        /// @return sqrtPriceX96 The square root of the price of the pool, in Q96 precision.
        /// @return tick The current tick of the pool.
        /// @return protocolFee The protocol fee of the pool.
        /// @return lpFee The swap fee of the pool.
        function getSlot0(PoolId poolId)
            external
            view
            returns (uint160 sqrtPriceX96, int24 tick, uint24 protocolFee, uint24 lpFee);

        /// @notice Retrieves the tick information of a pool at a specific tick.
        /// @dev Corresponds to pools[poolId].ticks[tick]
        /// @param poolId The ID of the pool.
        /// @param tick The tick to retrieve information for.
        /// @return liquidityGross The total position liquidity that references this tick
        /// @return liquidityNet The amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left)
        /// @return feeGrowthOutside0X128 fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
        /// @return feeGrowthOutside1X128 fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
        function getTickInfo(PoolId poolId, int24 tick)
            external
            view
            returns (
                uint128 liquidityGross,
                int128 liquidityNet,
                uint256 feeGrowthOutside0X128,
                uint256 feeGrowthOutside1X128
            );

        /// @notice Retrieves the liquidity information of a pool at a specific tick.
        /// @dev Corresponds to pools[poolId].ticks[tick].liquidityGross and pools[poolId].ticks[tick].liquidityNet. A more gas efficient version of getTickInfo
        /// @param poolId The ID of the pool.
        /// @param tick The tick to retrieve liquidity for.
        /// @return liquidityGross The total position liquidity that references this tick
        /// @return liquidityNet The amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left)
        function getTickLiquidity(PoolId poolId, int24 tick)
            external
            view
            returns (uint128 liquidityGross, int128 liquidityNet);

        /// @notice Retrieves the fee growth outside a tick range of a pool
        /// @dev Corresponds to pools[poolId].ticks[tick].feeGrowthOutside0X128 and pools[poolId].ticks[tick].feeGrowthOutside1X128. A more gas efficient version of getTickInfo
        /// @param poolId The ID of the pool.
        /// @param tick The tick to retrieve fee growth for.
        /// @return feeGrowthOutside0X128 fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
        /// @return feeGrowthOutside1X128 fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
        function getTickFeeGrowthOutside(PoolId poolId, int24 tick)
            external
            view
            returns (uint256 feeGrowthOutside0X128, uint256 feeGrowthOutside1X128);

        /// @notice Retrieves the global fee growth of a pool.
        /// @dev Corresponds to pools[poolId].feeGrowthGlobal0X128 and pools[poolId].feeGrowthGlobal1X128
        /// @param poolId The ID of the pool.
        /// @return feeGrowthGlobal0 The global fee growth for token0.
        /// @return feeGrowthGlobal1 The global fee growth for token1.
        function getFeeGrowthGlobals(PoolId poolId)
            external
            view
            returns (uint256 feeGrowthGlobal0, uint256 feeGrowthGlobal1);

        /// @notice Retrieves the total liquidity of a pool.
        /// @dev Corresponds to pools[poolId].liquidity
        /// @param poolId The ID of the pool.
        /// @return liquidity The liquidity of the pool.
        function getLiquidity(PoolId poolId) external view returns (uint128 liquidity);

        /// @notice Retrieves the tick bitmap of a pool at a specific tick.
        /// @dev Corresponds to pools[poolId].tickBitmap[tick]
        /// @param poolId The ID of the pool.
        /// @param tick The tick to retrieve the bitmap for.
        /// @return tickBitmap The bitmap of the tick.
        function getTickBitmap(PoolId poolId, int16 tick) external view returns (uint256 tickBitmap);

        /// @notice Retrieves the position info without needing to calculate the `positionId`.
        /// @dev Corresponds to pools[poolId].positions[positionId]
        /// @param poolId The ID of the pool.
        /// @param owner The owner of the liquidity position.
        /// @param tickLower The lower tick of the liquidity range.
        /// @param tickUpper The upper tick of the liquidity range.
        /// @param salt The bytes32 randomness to further distinguish position state.
        /// @return liquidity The liquidity of the position.
        /// @return feeGrowthInside0LastX128 The fee growth inside the position for token0.
        /// @return feeGrowthInside1LastX128 The fee growth inside the position for token1.
        function getPositionInfo(PoolId poolId, address owner, int24 tickLower, int24 tickUpper, bytes32 salt)
            external
            view
            returns (uint128 liquidity, uint256 feeGrowthInside0LastX128, uint256 feeGrowthInside1LastX128);

        /// @notice Retrieves the position information of a pool at a specific position ID.
        /// @dev Corresponds to pools[poolId].positions[positionId]
        /// @param poolId The ID of the pool.
        /// @param positionId The ID of the position.
        /// @return liquidity The liquidity of the position.
        /// @return feeGrowthInside0LastX128 The fee growth inside the position for token0.
        /// @return feeGrowthInside1LastX128 The fee growth inside the position for token1.
        function getPositionInfo(PoolId poolId, bytes32 positionId)
            external
            view
            returns (uint128 liquidity, uint256 feeGrowthInside0LastX128, uint256 feeGrowthInside1LastX128);

        /// @notice Retrieves the liquidity of a position.
        /// @dev Corresponds to pools[poolId].positions[positionId].liquidity. More gas efficient for just retrieving liquidity as compared to getPositionInfo
        /// @param poolId The ID of the pool.
        /// @param positionId The ID of the position.
        /// @return liquidity The liquidity of the position.
        function getPositionLiquidity(PoolId poolId, bytes32 positionId) external view returns (uint128 liquidity);

        /// @notice Calculate the fee growth inside a tick range of a pool
        /// @dev pools[poolId].feeGrowthInside0LastX128 in Position.Info is cached and can become stale. This function will calculate the up to date feeGrowthInside
        /// @param poolId The ID of the pool.
        /// @param tickLower The lower tick of the range.
        /// @param tickUpper The upper tick of the range.
        /// @return feeGrowthInside0X128 The fee growth inside the tick range for token0.
        /// @return feeGrowthInside1X128 The fee growth inside the tick range for token1.
        function getFeeGrowthInside(PoolId poolId, int24 tickLower, int24 tickUpper)
            external
            view
            returns (uint256 feeGrowthInside0X128, uint256 feeGrowthInside1X128);
    }

}
