use alloy::sol;

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveI128_2 {
            function get_dy(int128,int128,uint256) external view returns (uint256);
            function calc_withdraw_one_coin(uint256,int128) external view returns (uint256);
            function calc_token_amount(uint256[2],bool) external view returns (uint256);
            function exchange(int128,int128,uint256,uint256) external;
            function remove_liquidity_one_coin(uint256,int128,uint256) external;
            function add_liquidity(uint256[2],uint256) external;
    }

}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveI128_2_To_Meta {
            function get_dy(int128,int128,uint256) external view returns (uint256);
            function get_dy_underlying(int128,int128,uint256) external view returns (uint256);
            function calc_withdraw_one_coin(uint256,int128) external view returns (uint256);
            function calc_token_amount(uint256[2],bool) external view returns (uint256);
            function exchange(int128,int128,uint256,uint256,address) external;
            function exchange_underlying(int128,int128,uint256,uint256,address) external;
            function remove_liquidity_one_coin(uint256,int128,uint256) external;
            function add_liquidity(uint256[2],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveI128_2_To {
            function get_dy(int128,int128,uint256) external view returns (uint256);
            function get_dx(int128,int128,uint256) external view returns (uint256);
            function calc_withdraw_one_coin(uint256,int128) external view returns (uint256);
            function calc_token_amount(uint256[2],bool) external view returns (uint256);
            function exchange(int128,int128,uint256,uint256,address) external;
            function remove_liquidity_one_coin(uint256,int128,uint256) external;
            function add_liquidity(uint256[2],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveI128_3 {
        function get_dy(int128,int128,uint256) external view returns (uint256);
        function calc_withdraw_one_coin(uint256,int128) external view returns (uint256);
        function calc_token_amount(uint256[3],bool) external view returns (uint256);
        function exchange(int128,int128,uint256,uint256) external;
        function remove_liquidity_one_coin(uint256,int128,uint256) external;
        function add_liquidity(uint256[3],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveI128_4 {
        function get_dy(int128,int128,uint256) external view returns (uint256);
        function calc_token_amount(uint256[4],bool) external view returns (uint256);
        function exchange(int128,int128,uint256,uint256) external;
        function add_liquidity(uint256[4],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveU256_2_To{
        function get_dy(uint256,uint256,uint256) external view returns (uint256);
        function calc_withdraw_one_coin(uint256,int128) external view returns (uint256);
        function calc_token_amount(uint256[2],bool) external view returns (uint256);
        function exchange(uint256,uint256,uint256,uint256,address) external;
        function remove_liquidity_one_coin(uint256,uint128,uint256) external;
        function add_liquidity(uint256[2],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveU256_2{
        function get_dy(uint256,uint256,uint256) external view returns (uint256);
        function calc_withdraw_one_coin(uint256,uint256) external view returns (uint256);
        function calc_token_amount(uint256[2]) external view returns (uint256);
        function exchange(uint256,uint256,uint256,uint256) external;
        function remove_liquidity_one_coin(uint256,uint256,uint256) external;
        function add_liquidity(uint256[2],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveU256_2_Eth_To {
        function get_dy(uint256,uint256,uint256) external view returns (uint256);
        function get_dx(uint256,uint256,uint256) external view returns (uint256);
        function calc_withdraw_one_coin(uint256,int128) external view returns (uint256);
        function calc_token_amount(uint256[2],bool) external view returns (uint256);
        function exchange(uint256,uint256,uint256,uint256,bool,address) external;
        function remove_liquidity_one_coin(uint256,uint128,uint256) external;
        function add_liquidity(uint256[2],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveU256_3_Eth{
        function get_dy(uint256,uint256,uint256) external view returns (uint256);
        function calc_withdraw_one_coin(uint256,uint256) external view returns (uint256);
        function calc_token_amount(uint256[3],bool) external view returns (uint256);
        function exchange(uint256,uint256,uint256,uint256,bool) external;
        function remove_liquidity_one_coin(uint256,uint256,uint256) external;
        function add_liquidity(uint256[3],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveU256_3_Eth_To{
        function get_dy(uint256,uint256,uint256) external view returns (uint256);
        function get_dx(uint256,uint256,uint256) external view returns (uint256);
        function calc_token_amount(uint256[3],bool) external view returns (uint256);
        function calc_withdraw_one_coin(uint256,int128) external  view returns (uint256);
        function exchange(uint256,uint256,uint256,uint256,bool,address) external;
        function remove_liquidity_one_coin(uint256,uint256,uint256) external;
        function add_liquidity(uint256[3],uint256) external;
    }
}

sol! {
    #[sol(abi = true, rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface ICurveU256_3_Eth_To2 {
        function get_dy(uint256,uint256,uint256) external view returns (uint256);
        function get_dx(uint256,uint256,uint256) external view returns (uint256);
        function calc_token_amount(uint256[3], bool) external view returns (uint256);
        function calc_withdraw_one_coin(uint256,uint256) external  view returns (uint256);
        function exchange(uint256,uint256,uint256,uint256,bool,address) external;
        function remove_liquidity_one_coin(uint256,uint256,uint256) external;
        function add_liquidity(uint256[3],uint256) external;
    }
}
