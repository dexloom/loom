use alloy_sol_macro::sol;

sol! {

    #[sol(abi=true,rpc)]
    #[derive(Debug, PartialEq, Eq)]
    interface IUniswapV2Router {
        function getAmountOut(uint amountIn, uint reserveIn, uint reserveOut)
            public
            pure
            virtual
            override
            returns (uint amountOut);

        function getAmountIn(uint amountOut, uint reserveIn, uint reserveOut)
            public
            pure
            virtual
            override
            returns (uint amountIn);
    }
}
