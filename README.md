[![Release](https://github.com/frkn-dev/pony/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/frkn-dev/pony/actions/workflows/release.yml) [![Pony Build](https://github.com/frkn-dev/pony/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/frkn-dev/pony/actions/workflows/rust.yml)

# Pony - a cluster platform for Xray/Wiregard

## Architecture

Contains parts

- agent — manages Xray/Wireguard(in progress) connections/users/metrics
- api — manages cluster of servers, gets API calls and send commands to servers
- bot — https://github.com/frkn-dev/vpn-bot
- utils — helper to work/debug Bin messages

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
export VLESS_GRPC_INBOUND_ENABLED=true
export VLESS_XTLS_INBOUND_ENABLED=true
export VMESS_INBOUND_ENABLED=true
export XRAY_API_PORT=55555
export VMESS_HOST="google.com"
export VMESS_PORT=80
export VLESS_XTLS_PORT=8443
export VLESS_GRPC_PORT=8445
export VLESS_SERVER_NAME="discordapp.com"
export VLESS_DEST="discordapp.com:443"

# WG Settings
export WG_ENABLED="true"
export WG_PORT="51820"
export WG_NETWORK="10.10.0.0/16"
export WG_ADDRES="10.10.0.1"
export WG_INTERFACE=wg0
export WG_DNS="1.1.1.1 8.8.8.8"
export WG_PRIVKEY=2Gq3WPXHIFLcLsxUo/kP1PN+sY5kSfpO2V0YTCw0Uls=
export WG_PUBKEY=5mpYRvYWL1Z4SQSx5k3xuVBhpWF1zIBWMDn4Jp1TuUs=


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

Note: The script will install Wireguard Server, Xray Core and Pony Agent automagically.

### Features

- Standalone Agent — can run without external dependencies: just WG or/and Xray Core and a config file.
- Automatic Xray Config Parsing — reads xray-config.json to fetch inbounds and settings automatically.
- Low Resource Usage — works perfectly on low-cost 1 CPU ($5 VPS) machines.
- Protocol Support — handles VLESS XTLS, VLESS gRPC, VMESS, and Wireguard connections.
- Dockerized Infrastructure — Bot and API run inside Docker containers.
- Cluster Management — API manages users and nodes across the entire cluster.
- Node Health Monitoring — API periodically checks the health and status of all connected nodes.
- Metrics System — system and logic metrics are collected in Graphite format and stored in Clickhouse for analytics.
- Trial User Support — supports trial users with daily traffic limitation control.

# Utils — CLI Tool for Working with rkyv Messages and ZeroMQ

## Overview

utils is a command-line utility to generate, verify, and send binary rkyv messages used in the messaging system over ZeroMQ. It helps to easily create valid messages from JSON, serialize them to binary files, and publish them on a ZeroMQ topic.
Usage

Generate binary message from JSON

```
cargo run --bin utils gen -i path/to/input.json -o path/to/output.bin

    -i — path to the JSON file describing the message

    -o — path to save the generated binary file

    The JSON must match the Message struct format.
```

Send a binary message to ZeroMQ publisher

```
cargo run --bin utils send -t topic -i path/to/message.bin -a tcp://127.0.0.1:3000

    -t — topic (channel) to publish on

    -i — path to the binary message file

    -a — ZeroMQ publisher address
```

Combined mode: generate and send in one step

    cargo run --bin utils all -i path/to/input.json -b path/to/output.bin -t topic -a tcp://127.0.0.1:3000`

Generates binary from JSON, saves it, and immediately publishes it to the given topic and address.

```
Example JSON for Message

{
  "conn_id": "9658b391-01cb-4031-a3f5-6cbdd749bcff",
  "action": "Create",
  "tag": "Wireguard",
  "wg": {
    "keys": {
      "privkey": "privatekeyhere",
      "pubkey": "publickeyhere"
    },
    "address": {
      "ip": "10.10.0.24",
      "cidr": 32
    }
  },
  "password": null
}
```
