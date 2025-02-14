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
    interface IUniswapV4Pool {
        struct Slot0 {
            // the current price
            uint160 sqrtPriceX96;
            // the current tick
            int24 tick;
            // protocol swap fee represented as integer denominator (1/x), taken as a % of the LP swap fee
            // upper 8 bits are for 1->0, and the lower 8 are for 0->1
            // the minimum permitted denominator is 4 - meaning the maximum protocol fee is 25%
            // granularity is increments of 0.38% (100/type(uint8).max)
            uint16 protocolFee;
            // used for the swap fee, either static at initialize or dynamic via hook
            uint24 swapFee;
        }

        struct TickInfo {
        // the total position liquidity that references this tick
            uint128 liquidityGross;
            // amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left),
            int128 liquidityNet;
            // fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
            // only has relative meaning, not absolute — the value depends on when the tick is initialized
            uint256 feeGrowthOutside0X128;
            uint256 feeGrowthOutside1X128;
        }



        struct SwapParams {
            int24 tickSpacing;
            bool zeroForOne;
            int256 amountSpecified;
            uint160 sqrtPriceLimitX96;
        }

        struct ModifyPositionParams {
        // the address that owns the position
            address owner;
            // the lower and upper tick of the position
            int24 tickLower;
            int24 tickUpper;
            // any change in liquidity
            int128 liquidityDelta;
            // the spacing between ticks
            int24 tickSpacing;
        }

    }


    #[derive(Debug, PartialEq, Eq)]
    interface IUniswapV4LockCallback {
        /// @notice Called by the pool manager on `msg.sender` when a lock is acquired
        /// @param lockCaller The address that originally locked the PoolManager
        /// @param data The data that was passed to the call to lock
        /// @return Any data that you want to be returned from the lock call
        function lockAcquired(address lockCaller, bytes calldata data) external returns (bytes memory);
    }




    #[derive(Debug, PartialEq, Eq)]
    interface IUniswapV4PoolManager {
    /// @notice Thrown when currencies touched has exceeded max of 256
        error MaxCurrenciesTouched();

        /// @notice Thrown when a currency is not netted out after a lock
        error CurrencyNotSettled();

        /// @notice Thrown when trying to interact with a non-initialized pool
        error PoolNotInitialized();

        /// @notice Thrown when a function is called by an address that is not the current locker
        /// @param locker The current locker
        /// @param currentHook The most recently called hook
        error LockedBy(address locker, address currentHook);

        /// @notice The ERC1155 being deposited is not the Uniswap ERC1155
        error NotPoolManagerToken();

        /// @notice Pools are limited to type(int16).max tickSpacing in #initialize, to prevent overflow
        error TickSpacingTooLarge();
        /// @notice Pools must have a positive non-zero tickSpacing passed to #initialize
        error TickSpacingTooSmall();

        /// @notice PoolKey must have currencies where address(currency0) < address(currency1)
        error CurrenciesOutOfOrderOrEqual();

        /// @notice Emitted when a new pool is initialized
        /// @param id The abi encoded hash of the pool key struct for the new pool
        /// @param currency0 The first currency of the pool by address sort order
        /// @param currency1 The second currency of the pool by address sort order
        /// @param fee The fee collected upon every swap in the pool, denominated in hundredths of a bip
        /// @param tickSpacing The minimum number of ticks between initialized ticks
        /// @param hooks The hooks contract address for the pool, or address(0) if none
        event Initialize(
            PoolId indexed id,
            Currency indexed currency0,
            Currency indexed currency1,
            uint24 fee,
            int24 tickSpacing,
            Hooks hooks
        );

        /// @notice Emitted when a liquidity position is modified
        /// @param id The abi encoded hash of the pool key struct for the pool that was modified
        /// @param sender The address that modified the pool
        /// @param tickLower The lower tick of the position
        /// @param tickUpper The upper tick of the position
        /// @param liquidityDelta The amount of liquidity that was added or removed
        event ModifyLiquidity(
            PoolId indexed id, address indexed sender, int24 tickLower, int24 tickUpper, int256 liquidityDelta
        );

        /// @notice Emitted for swaps between currency0 and currency1
        /// @param id The abi encoded hash of the pool key struct for the pool that was modified
        /// @param sender The address that initiated the swap call, and that received the callback
        /// @param amount0 The delta of the currency0 balance of the pool
        /// @param amount1 The delta of the currency1 balance of the pool
        /// @param sqrtPriceX96 The sqrt(price) of the pool after the swap, as a Q64.96
        /// @param liquidity The liquidity of the pool after the swap
        /// @param tick The log base 1.0001 of the price of the pool after the swap
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

        event ProtocolFeeUpdated(PoolId indexed id, uint16 protocolFee);

        event DynamicSwapFeeUpdated(PoolId indexed id, uint24 dynamicSwapFee);

        /// @notice Returns the constant representing the maximum tickSpacing for an initialized pool key
        function MAX_TICK_SPACING() external view returns (int24);

        /// @notice Returns the constant representing the minimum tickSpacing for an initialized pool key
        function MIN_TICK_SPACING() external view returns (int24);

        /// @notice Get the current value in slot0 of the given pool
        function getSlot0(PoolId id) external view returns (uint160 sqrtPriceX96, int24 tick, uint16 protocolFee);

        /// @notice Get the current value of liquidity of the given pool
        function getLiquidity(PoolId id) external view returns (uint128 liquidity);

        /// @notice Get the current value of liquidity for the specified pool and position
        function getLiquidity(PoolId id, address owner, int24 tickLower, int24 tickUpper)
            external
            view
            returns (uint128 liquidity);

        /// @notice Getter for TickInfo for the given poolId and tick
        function getPoolTickInfo(PoolId id, int24 tick) external view returns (PoolTickInfo memory);

        /// @notice Getter for the bitmap given the poolId and word position
        function getPoolBitmapInfo(PoolId id, int16 word) external view returns (uint256 tickBitmap);

        /// @notice Get the position struct for a specified pool and position
        function getPosition(PoolId id, address owner, int24 tickLower, int24 tickUpper)
            external
            view
            returns (PositionInfo memory position);

        /// @notice Returns the reserves for a given ERC20 currency
        function reservesOf(Currency currency) external view returns (uint256);

        /// @notice Returns the locker in the ith position of the locker queue.
        function getLock(uint256 i) external view returns (address locker, address lockCaller);

        /// @notice Returns the length of the lockers array, which is the number of locks open on the PoolManager.
        function getLockLength() external view returns (uint256 _length);

        /// @notice Returns the most recently called hook.
        function getCurrentHook() external view returns (Hooks _currentHook);

        /// @notice Returns the number of nonzero deltas open on the PoolManager that must be zerod by the close of the initial lock.
        function getLockNonzeroDeltaCount() external view returns (uint256 _nonzeroDeltaCount);

        /// @notice Initialize the state for a given pool ID
        function initialize(PoolKey memory key, uint160 sqrtPriceX96, bytes calldata hookData)
            external
            returns (int24 tick);

        /// @notice Get the current delta for a locker in the given currency
        /// @param locker The address of the locker
        /// @param currency The currency for which to lookup the delta
        function currencyDelta(address locker, Currency currency) external view returns (int256);

        /// @notice All operations go through this function
        /// @param lockTarget The address to call the callback on
        /// @param data Any data to pass to the callback, via `ILockCallback(msg.sender).lockAcquired(data)`
        /// @return The data returned by the call to `ILockCallback(msg.sender).lockAcquired(data)`
        function lock(address lockTarget, bytes calldata data) external payable returns (bytes memory);

        struct ModifyLiquidityParams {
            // the lower and upper tick of the position
            int24 tickLower;
            int24 tickUpper;
            // how to modify the liquidity
            int256 liquidityDelta;
        }

        /// @notice Modify the liquidity for the given pool
        /// @dev Poke by calling with a zero liquidityDelta
        /// @param key The pool to modify liquidity in
        /// @param params The parameters for modifying the liquidity
        /// @param hookData Any data to pass to the callback, via `ILockCallback(msg.sender).lockAcquired(data)`
        /// @return delta The balance delta of the liquidity
        function modifyLiquidity(PoolKey memory key, ModifyLiquidityParams memory params, bytes calldata hookData)
            external
            returns (BalanceDelta);

        struct SwapParams {
            bool zeroForOne;
            int256 amountSpecified;
            uint160 sqrtPriceLimitX96;
        }

        /// @notice Swap against the given pool
        function swap(PoolKey memory key, SwapParams memory params, bytes calldata hookData)
            external
            returns (BalanceDelta);

        /// @notice Donate the given currency amounts to the pool with the given pool key
        function donate(PoolKey memory key, uint256 amount0, uint256 amount1, bytes calldata hookData)
            external
            returns (BalanceDelta);

        /// @notice Called by the user to net out some value owed to the user
        /// @dev Can also be used as a mechanism for _free_ flash loans
        function take(Currency currency, address to, uint256 amount) external;

        /// @notice Called by the user to move value into ERC6909 balance
        function mint(address to, uint256 id, uint256 amount) external;

        /// @notice Called by the user to move value from ERC6909 balance
        function burn(address from, uint256 id, uint256 amount) external;

        /// @notice Called by the user to pay what is owed
        function settle(Currency token) external payable returns (uint256 paid);

        /// @notice Sets the protocol's swap fee for the given pool
        /// Protocol fees are always a portion of the LP swap fee that is owed. If that fee is 0, no protocol fees will accrue even if it is set to > 0.
        function setProtocolFee(PoolKey memory key) external;

        /// @notice Updates the pools swap fees for the a pool that has enabled dynamic swap fees.
        function updateDynamicSwapFee(PoolKey memory key) external;

        /// @notice Called by external contracts to access granular pool state
        /// @param slot Key of slot to sload
        /// @return value The value of the slot as bytes32
        function extsload(bytes32 slot) external view returns (bytes32 value);

        /// @notice Called by external contracts to access granular pool state
        /// @param slot Key of slot to start sloading from
        /// @param nSlots Number of slots to load into return value
        /// @return value The value of the sload-ed slots concatenated as dynamic bytes
        function extsload(bytes32 slot, uint256 nSlots) external view returns (bytes memory value);
    }

    #[derive(Debug, PartialEq, Eq)]
    interface IHooks {
        /// @notice The hook called before the state of a pool is initialized
        /// @param sender The initial msg.sender for the initialize call
        /// @param key The key for the pool being initialized
        /// @param sqrtPriceX96 The sqrt(price) of the pool as a Q64.96
        /// @param hookData Arbitrary data handed into the PoolManager by the initializer to be be passed on to the hook
        /// @return bytes4 The function selector for the hook
        function beforeInitialize(address sender, PoolKey calldata key, uint160 sqrtPriceX96, bytes calldata hookData)
            external
            returns (bytes4);

        /// @notice The hook called after the state of a pool is initialized
        /// @param sender The initial msg.sender for the initialize call
        /// @param key The key for the pool being initialized
        /// @param sqrtPriceX96 The sqrt(price) of the pool as a Q64.96
        /// @param tick The current tick after the state of a pool is initialized
        /// @param hookData Arbitrary data handed into the PoolManager by the initializer to be be passed on to the hook
        /// @return bytes4 The function selector for the hook
        function afterInitialize(
            address sender,
            PoolKey calldata key,
            uint160 sqrtPriceX96,
            int24 tick,
            bytes calldata hookData
        ) external returns (bytes4);

        /// @notice The hook called before liquidity is added
        /// @param sender The initial msg.sender for the add liquidity call
        /// @param key The key for the pool
        /// @param params The parameters for adding liquidity
        /// @param hookData Arbitrary data handed into the PoolManager by the liquidty provider to be be passed on to the hook
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
        /// @param hookData Arbitrary data handed into the PoolManager by the liquidty provider to be be passed on to the hook
        /// @return bytes4 The function selector for the hook
        function afterAddLiquidity(
            address sender,
            PoolKey calldata key,
            IPoolManagerModifyLiquidityParams calldata params,
            BalanceDelta delta,
            bytes calldata hookData
        ) external returns (bytes4);

        /// @notice The hook called before liquidity is removed
        /// @param sender The initial msg.sender for the remove liquidity call
        /// @param key The key for the pool
        /// @param params The parameters for removing liquidity
        /// @param hookData Arbitrary data handed into the PoolManager by the liquidty provider to be be passed on to the hook
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
        /// @param hookData Arbitrary data handed into the PoolManager by the liquidty provider to be be passed on to the hook
        /// @return bytes4 The function selector for the hook
        function afterRemoveLiquidity(
            address sender,
            PoolKey calldata key,
            IPoolManagerModifyLiquidityParams calldata params,
            BalanceDelta delta,
            bytes calldata hookData
        ) external returns (bytes4);

        /// @notice The hook called before a swap
        /// @param sender The initial msg.sender for the swap call
        /// @param key The key for the pool
        /// @param params The parameters for the swap
        /// @param hookData Arbitrary data handed into the PoolManager by the swapper to be be passed on to the hook
        /// @return bytes4 The function selector for the hook
        function beforeSwap(
            address sender,
            PoolKey calldata key,
            IPoolManagerSwapParams calldata params,
            bytes calldata hookData
        ) external returns (bytes4);

        /// @notice The hook called after a swap
        /// @param sender The initial msg.sender for the swap call
        /// @param key The key for the pool
        /// @param params The parameters for the swap
        /// @param delta The amount owed to the locker (positive) or owed to the pool (negative)
        /// @param hookData Arbitrary data handed into the PoolManager by the swapper to be be passed on to the hook
        /// @return bytes4 The function selector for the hook
        function afterSwap(
            address sender,
            PoolKey calldata key,
            IPoolManagerSwapParams calldata params,
            BalanceDelta delta,
            bytes calldata hookData
        ) external returns (bytes4);

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

}
