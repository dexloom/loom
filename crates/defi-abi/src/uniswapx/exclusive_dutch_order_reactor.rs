use alloy_sol_macro::sol;

sol! {
    // FILE: ReactorEvents.sol
    interface ReactorEvents {
        event Fill(bytes32 indexed orderHash, address indexed filler, address indexed swapper, uint256 nonce);
    }

    // FILE: IReactor.sol
    struct SignedOrder {
        bytes order;
        bytes sig;
    }

    interface IReactor {
        function execute(SignedOrder calldata order) external payable;
        function executeWithCallback(SignedOrder calldata order, bytes calldata callbackData) external payable;
        function executeBatch(SignedOrder[] calldata orders) external payable;
        function executeBatchWithCallback(SignedOrder[] calldata orders, bytes calldata callbackData) external payable;
    }
}
