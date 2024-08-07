use alloy_sol_types::sol;

sol! {

    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IWETH {
        function deposit() public payable;
        function withdraw(uint wad) public;
        function totalSupply() public view returns (uint);
        function balanceOf(address account) external view returns (uint256);
    }


}
