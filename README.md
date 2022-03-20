# Prism Protocol Contracts

This repository contains the source code for the first version of Prism Protocol smart contracts running on [Terra](https://terra.money) blockchain.

## Contracts

| Contract                                                 | Reference | Description                                                                                 |
| -------------------------------------------------------- | --------- | ------------------------------------------------------------------------------------------- |
| [`airdrop-registry`](./contracts/prism-airdrop-registry) | [doc]()   | Manages message fabricators for airdrop claims                                              |
| [`basset-vault`](./contracts/basset-vault)               | [doc]()   | Vault for bonding bAssets |
| [`collector`](./contracts/prism-collector)               | [doc]()   | Gathers protocol fees and distributes to xPRISM holders, manages reward swap logic          |
| [`delegator-rewards`](./contracts/delegator-rewards)     | [doc]()   | Contract for processing delegator rewards |
| [`gov`](./contracts/prism-gov)                           | [doc]()   | Governance contract for Prism Protocol, manages xPRISM minting and rewards                  |
| [`launch-pool`](./contracts/prism-launch-pool)           | [doc]()   | yLuna staking pool event                                                                    |
| [`lp-staking`](./contracts/prism-lp-staking)             | [doc]()   | Staking contract for PRISM LP tokens                                                        |
| [`reward-distribution`](./contracts/prism-reward-distribution) | [doc]()   | Distribute rewards appropriately among collector, yasset-staking, and yasset-staking-x contracts |
| [`vault`](./contracts/prism-vault)                       | [doc]()   | Manages luna delegations and yLuna/pLuna minting                                            |
| [`xprism-boost`](./contracts/prism-xprism-boost)         | [doc]()   | Allows users to lock xPRISM to farm AMPS                                                    |
| [`yasset-staking`](./contracts/prism-yasset-staking)     | [doc]()   | Staking contract for yAssets |
| [`yasset-staking-x`](./contracts/prism-yasset-staking-x) | [doc]()   | Autocompounding staking contract for yAssets |

## Bug Bounty

There is an active bug bounty program for Prism Smart Contracts on [Inmunefi](https://immunefi.com/bounty/prismprotocol/).

## Development

### Environment Setup

- Rust v1.44.1+
- `wasm32-unknown-unknown` target
- Docker

1. Install `rustup` via https://rustup.rs/

2. Run the following:

```sh
rustup default stable
rustup target add wasm32-unknown-unknown
```

3. Make sure [Docker](https://www.docker.com/) is installed

### Unit / Integration Tests

Each contract contains Rust unit and integration tests embedded within the contract source directories. You can run:

```sh
cargo unit-test
cargo integration-test
```

### Compiling

After making sure tests pass, you can compile each contract with the following:

```sh
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/cw1_subkeys.wasm .
ls -l cw1_subkeys.wasm
sha256sum cw1_subkeys.wasm
```

#### Production

For production builds, run the following:

```sh
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.11.5
```

or

```sh
sh build_release.sh
```

This performs several optimizations which can significantly reduce the final size of the contract binaries, which will be available inside the `artifacts/` directory.

## Formatting

Make sure you run `rustfmt` before creating a PR to the repo. You need to install the `nightly` version of `rustfmt`.

```sh
rustup toolchain install nightly
```

To run `rustfmt`,

```sh
cargo fmt
```

## Linting

You should run `clippy` also. This is a lint tool for rust. It suggests more efficient/readable code.
You can see [the clippy document](https://rust-lang.github.io/rust-clippy/master/index.html) for more information.
You need to install `nightly` version of `clippy`.

### Install

```sh
rustup toolchain install nightly
```

### Run

```sh
cargo clippy --all --all-targets -- -D warnings
```

## Testing

Developers are strongly encouraged to write unit tests for new code, and to submit new unit tests for old code. Unit tests can be compiled and run with: `cargo test --all`. For more details, please reference [Unit Tests](https://github.com/CodeChain-io/codechain/wiki/Unit-Tests).
