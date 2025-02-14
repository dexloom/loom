use alloy::sol;

sol! {
    #[sol(abi=true,rpc)]
    #[derive(Debug, PartialEq, Eq)]
   interface IERC20 {
       event Transfer(address indexed from, address indexed to, uint256 value);
       event Approval(address indexed owner, address indexed spender, uint256 value);

       function decimals() external view returns (uint256);
       function totalSupply() external view returns (uint256);
       function balanceOf(address account) external view returns (uint256);
       function transfer(address to, uint256 amount) external returns (bool);
       function allowance(address owner, address spender) external view returns (uint256);
       function approve(address spender, uint256 amount) external returns (bool);
       function transferFrom(address from, address to, uint256 amount) external returns (bool);
   }
}
