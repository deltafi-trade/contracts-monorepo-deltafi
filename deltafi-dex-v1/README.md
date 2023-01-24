# DeltaFi DEX V1 Smart Contract

The World's Most Efficient DEX, Powered by Intelligent Algorithms

## Development

Download or update the BPF-SDK by running:

### Testing

Unit tests contained within the project can be built via:

```bash
cargo test
```

Running bpf tests:

```bash
cargo test-bpf
```

bpf test trace:

```bash
$ RUST_BACKTRACE=full cargo test-bpf --test init_swap_pool
```

Run clippy
```
cargo clippy
```

### Test Coverage

Coverage is supported via:

```bash
$ cargo install cargo-tarpaulin
$ cargo tarpaulin -v
```

### Fuzz tests

Using the Rust version of `honggfuzz`, we "fuzz" the Farm Instructions

Install `honggfuzz` with:

```sh
cargo install honggfuzz
```

run fuzzing from `./fuzz` with:

```sh
cargo hfuzz run deltafi-swap-fuzz-farm
```

If the program crashes or errors, `honggfuzz` dumps a `.fuzz` file in the workspace,
so you can debug the failing input using:

```sh
cargo hfuzz run-debug deltafi-swap-fuzz-farm hfuzz_workspace/token-swap-instructions/*fuzz
```

This command attaches a debugger to the test, allowing you to easily see the
exact problem.

### Deployment

see readme in `lib/client/admin`.

## TODO

- [x] Implement [`get_virtual_price`](https://github.com/curvefi/curve-contract/blob/4aa3832a4871b1c5b74af7f130c5b32bdf703af5/contracts/pool-templates/base/SwapTemplateBase.vy#L241)
- [x] Implement [`remove_liquidity_one_coin`](https://github.com/curvefi/curve-contract/blob/4aa3832a4871b1c5b74af7f130c5b32bdf703af5/contracts/pool-templates/base/SwapTemplateBase.vy#L695)
- [x] [Admin functions](https://github.com/curvefi/curve-contract/blob/4aa3832a4871b1c5b74af7f130c5b32bdf703af5/contracts/pool-templates/base/SwapTemplateBase.vy#L732)

## Out of scope for v1

- [ ] Generalize swap pool to support `n` tokens
- [ ] Implement [`remove_liquidity_imbalance`](https://github.com/curvefi/curve-contract/blob/4aa3832a4871b1c5b74af7f130c5b32bdf703af5/contracts/pool-templates/base/SwapTemplateBase.vy#L539)
