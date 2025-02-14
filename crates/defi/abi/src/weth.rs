use alloy::sol;

sol! {

    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IWETH {
        event Approval(address indexed src, address indexed guy, uint256 wad);
        event Transfer(address indexed src, address indexed dst, uint256 wad);
        event Deposit(address indexed dst, uint256 wad);
        event Withdrawal(address indexed src, uint256 wad);

        function deposit() public payable;
        function withdraw(uint wad) public;
        function totalSupply() public view returns (uint);
        function balanceOf(address account) external view returns (uint256);
    }


}
