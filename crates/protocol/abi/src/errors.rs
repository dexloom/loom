use alloy_sol_types::{sol, SolCall, SolInterface};

sol!{
    #[derive(Debug, PartialEq, Eq)]
    interface IAbiErrors {
        error AllowanceExpired(uint256 deadline);
        error InsufficientAllowance(uint256 amount);
        error ExcessiveInvalidation();
        error ContractLocked();
        error InsufficientToken();
        error InsufficientETH();
        error InvalidBips();
        error InvalidSpender();
        error V3InvalidSwap();
        error V3TooLittleReceived();
        error V3TooMuchRequested();
        error V3InvalidAmountOut();
        error V3InvalidCaller();
        error V2TooLittleReceived();
        error V2TooMuchRequested();
        error V2InvalidPath();
        error ExecutionFailed(uint256 commandIndex, bytes message);
        error ETHNotAccepted();
        error UnsafeCast();
        error TransactionDeadlinePassed();
        error FromAddressIsNotOwner();
        error LengthMismatch();
        error UnableToClaim();
        error InvalidCommandType(uint256 commandType);
        error BuyPunkFailed();
        error InvalidOwnerERC721();
        error InvalidOwnerERC1155();
        error BalanceTooLow();
        error InvalidReserves();
        error InvalidPath();
        error Error(string);
        error Fallback(bytes);
        error Unknown(bytes);
    }
}

