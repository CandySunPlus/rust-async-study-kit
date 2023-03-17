RUST Async study kit
---

## slow-read

An example for wrap AsyncRead

```
cargo build --release --bin slow-read
```

## custom-async

An example for custom executor, future (Task) and reactor 

```
cargo build --release --bin custom-async
```

## simple-executor

An example for custom a simple executor with a sync channel

```
cargo build --release --bin simple-executor
```

## simple-runtime

An example for custom a mini runtime with `polling` crate

```
cargo build --release --bin simple-runtime
```
