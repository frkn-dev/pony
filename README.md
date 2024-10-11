## Pony - monitoring agent for Xray/Wiregard
(pony means ponytoring)

Collects metrics on lowlevel and sends to Carbon 

Works with [carbon-clickhouse](https://github.com/frkn-dev/graphite-clickhouse-tldr) stack

```build
cargo build --release
```

```test
cargo test
```


Prerequisites - [cross](https://github.com/cross-rs/cross)

```crosscompile
cross build --target x86_64-unknown-linux-gnu 
```
