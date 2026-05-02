[![Release](https://github.com/frkn-dev/fcore/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/frkn-dev/fcore/actions/workflows/release.yml) [![Fc0re Build](https://github.com/frkn-dev/pony/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/frkn-dev/fcore/actions/workflows/rust.yml)

# Fc0re - a cluster platform for Xray/Shadowsocks/Hysteria2/Wireguard/Amnezia-Wireguard/MTproto

Fc0re is a lightweight control plane and orchestration platform for modern proxy protocols.
It simplifies the deployment and unified management of Xray, Shadowsocks, Hysteria2, MTproto, Wireguard and Amnezia-Wireguard servers,
providing a single pane of glass for your network infrastructure.

## Architecture

Contains parts

- node — manages Xray/Shadowsocks/Hysteria2/Wireguard/Amnezia-Wireguard/MTproto connections/users/metrics
- api — manages cluster of servers, gets API calls and send commands to servers using ZeroMQ PUB/SUB mechanizme
- auth — handles auth for Hysteri2 clients and trial API

### As dependencies the platfrom has

- ZeroMQ — communicating bus
- PostgreSQL — user and node data storage
- Xray Core
- Hysteria2
- Teleproxy (MTProxy)
- Wireguard
- Amnezia Wireguard
- Nginx — reverse proxy

### Features

- Standalone Node — can run without external dependencies.
- Automatic Xray Config Parsing — reads xray-config.json to fetch inbounds and settings automatically.
- Low Resource Usage — works perfectly on low-cost 1 CPU ($3 VPS) machines.
- Protocol Support — handles VLESS TCP, VLESS gRPC, VLESS Xhttp, Hysteria2, Wireguard and Amnezia Wireguard connections.
- Cluster Management — API manages users and nodes across the entire cluster.
- Node Health Monitoring — API periodically checks the health and status of all connected nodes.
- Metrics System — system and logic metrics are collected in Graphite format and stored in Clickhouse for analytics.
- Trial User Support — supports trial users.

## Getting Started

### Prerequisites

- **Rust** (nightly toolchain)
- **PostgreSQL** 17+
- **ZeroMQ** libraries installed on your system
- **Protobuf Compiler** (`protoc`)

### Installation & Build

1. **Clone the repository:**

```bash
git clone [https://github.com/frkn-dev/fcore.git](https://github.com/frkn-dev/fcore.git)
cd fcore
```

2. **Build all components:**

```bash
cargo build --release
```

3. **Configuration**

```bash
cp config-node-example.toml config-node.toml
cp config-api-example.toml config-api.toml
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.
