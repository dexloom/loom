# Getting started

## Checkout the repository
Clone the repository:
```sh
git clone git@github.com:dexloom/loom.git
```

## Setting up topology
Copy `config-example.toml` to `config.toml` and configure according to your setup.

## Updating private key encryption password
Private key encryption password is individual secret key that is generated automatically but can be replaced

It is located in ./crates/defi-entities/private.rs and looks like

```rust
pub const KEY_ENCRYPTION_PWD: [u8; 16] = [35, 48, 129, 101, 133, 220, 104, 197, 183, 159, 203, 89, 168, 201, 91, 130];
```

To change key encryption password run and replace content of KEY_ENCRYPTION_PWD

```sh
cargo run --bin keys generate-password  
```

To get encrypted key run:

```sh
cargo run --bin keys encrypt --key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
```

## Setup database
Install postgresql and create database and user.

Create user and db:
```shell
su - postgres
createuser loom
createdb loom
```

Run `psql` and update user and privileges:
```psql
alter user loom with encrypted password 'loom';
grant all privileges on database loom to loom;
create schema loom;
grant usage on schema loom to loom;
grant create on schema loom to loom;
\q
```

## Starting loom
```sh
DATA=<ENCRYPTED_PRIVATE_KEY> cargo run --bin loom
```

## Makefile
Makefile is shipped with following important commands:

- build - builds all binaries
- fmt - formats loom with rustfmt
- pre-release - check code with rustfmt and clippy
- clippy - check code with clippy

## Testing
Testing Loom requires two environment variables pointing at archive node:

- MAINNET_WS - websocket url of archive node
- MAINNET_HTTP - http url of archive node

To run tests:

```shell
make test
```