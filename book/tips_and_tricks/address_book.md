# Address book
The address book contain ofter used addresses to have a convenient way to access them. It is less error-prone and easier to read.

## Address types
Right now you will find `TokenAddress`, `FactoryAddress`, `PeripheryAddress` and other more specific address clusters for different protocols like `UniswapV2PoolAddress`.

## Example
Just import is using the `loom` or the dedicated `defi-address-book` crate.

```rust,ignore
use loom::eth::address_book::TokenAddress;

let weth_address = TokenAddress::WETH;
```