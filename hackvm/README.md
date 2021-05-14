# Hack VM

A hack virtual machine interpreter written in Rust.

## Building

### Running tests:

```
cargo test
```

### Running performance benchmarks:

This code uses [Criterion](https://github.com/bheisler/criterion.rs) for running performance benchmarks. You can run the benchmarks with:

```
cargo bench
```

### Building to Web Assembly

To generate the npm package used by the website, run the following:

```
wasm-pack build
```

This will generate an npm package in the `pkg` directory, complete with the
compiled `.wasm` file, typescript type definitions, and wrapper js code for
instantiating and running the emulator from javascript.
