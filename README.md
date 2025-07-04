[![Release](https://github.com/frkn-dev/pony/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/frkn-dev/pony/actions/workflows/release.yml) [![Pony Build](https://github.com/frkn-dev/pony/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/frkn-dev/pony/actions/workflows/rust.yml)

# Pony - a cluster platform for Xray/Wiregard

## Architecture

Contains 3 parts

- agent — manages Xray/Wireguard(in progress) connections/users/metrics
- api — manages cluster of servers, gets API calls and send commands to servers
- bot — https://github.com/frkn-dev/vpn-bot

### As dependencies the platfrom has

- PostgreSQL — user and node data storage
- Clickhouse — metrics storage
- ZeroMQ — communicating bus
- Carbon — metrics relay
- Xray Core — censorship avoiding platfrom
- Wireguard
- Nginx — reverse proxy

## Agent Installation

1. Create .env file

```
export XRAY_VERSION="v24.11.30"
export PONY_VERSION="v0.0.28-dev"
export INSTALL_DIR="/opt/vpn"
export XRAY_CONFIG_PATH="$INSTALL_DIR/xray-config.json"
export PONY_CONFIG_PATH="$INSTALL_DIR/config-agent.toml"

# Xray core settings
export XRAY_API_PORT=55555
export SHADOWSOCKS_PORT=1080
export VMESS_HOST="google.com"
export VMESS_PORT=80
export VLESS_XTLS_PORT=8443
export VLESS_GRPC_PORT=8445
export VLESS_SERVER_NAME="discordapp.com"
export VLESS_DEST="discordapp.com:443"

# Pony agent settings
export CARBON_ADDRESS="Carbon Relay Address:2003"
export ZMQ_ENDPOINT="tcp://Pony API Address:3000"
export ENV="dev"
export API_ENDPOINT="https://Pony API Address"
export API_TOKEN="supersecretoken"
export UUID="8381b11e-6f3a-4248-9ae7-c14343ef6b1e"

```

2. Run install script

```
source ./env
bash deploy/install
```

Note: The script will install Xray Core and Pony Agent automatically.

### Features

- Standalone Agent — can run without external dependencies: just Xray Core and a config file.
- Automatic Xray Config Parsing — reads xray-config.json to fetch inbounds and settings automatically.
- Low Resource Usage — works perfectly on low-cost 1 CPU ($5 VPS) machines.
- Protocol Support — handles VLESS XTLS, VLESS gRPC, VMESS, and Shadowsocks connections.
- Dockerized Infrastructure — Bot and API run inside Docker containers.
- Cluster Management — API manages users and nodes across the entire cluster.
- Node Health Monitoring — API periodically checks the health and status of all connected nodes.
- Metrics System — system and logic metrics are collected in Graphite format and stored in Clickhouse for analytics.
- Trial User Support — supports trial users with daily traffic limitation control.
