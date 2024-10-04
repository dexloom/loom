use alloy_sol_macro::sol;

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug)]
    contract AMPLRebaser {
        uint256 public last_ampl_supply;
        uint256 public last_rebase_call;

        event Rebase(uint256 old_supply, uint256 new_supply);
        function rebase();
    }
}
