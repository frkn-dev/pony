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



```vless client connect example
vless://<uuid>@<server_ip>:<port>?security=reality&type=grpc&headerType=&serviceName=<grpc_service_name>&authority=&mode=gun&sni=<server_name>&fp=<fingerprint>&pbk=<public_key>&sid=<short_id>#<name>

```

```vmess client connect example 
base64 
vmess://eyJhZGQiOiAiSVAgQUREUiIsICJhaWQiOiAiMCIsICJob3N0IjogImdvb2dsZS5jb20iLCAiaWQiOiAiVVVJRCIsICJuZXQiOiAidGNwIiwgInBhdGgiOiAiLyIsICJwb3J0IjogPFZtZXNzIFBPUlQ+LCAicHMiOiAiVGhlIG5hbWUgaW4gY2xpZW50IiwgInNjeSI6ICJhdXRvIiwgInRscyI6ICJub25lIiwgInR5cGUiOiAiaHR0cCIsICJ2IjogIjIifQo=

decoded:

{"add": "IP ADDR", "aid": "0", "host": "google.com", "id": "UUID", "net": "tcp", "path": "/", "port": <Vmess PORT>, "ps": "The name in client", "scy": "auto", "tls": "none", "type": "http", "v": "2"}

```
