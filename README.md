[![Release](https://github.com/frkn-dev/pony/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/frkn-dev/pony/actions/workflows/release.yml) [![Pony Build](https://github.com/frkn-dev/pony/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/frkn-dev/pony/actions/workflows/rust.yml)

# Pony - a cluster platform for Xray/Shadowsocks/Hysteria2/Wireguard/MTproto

Pony is a lightweight control plane and orchestration platform for modern proxy protocols. It simplifies the deployment and unified management of Xray, Shadowsocks, Hysteria2, MTproto and Wireguard servers, providing a single pane of glass for your network infrastructure.

## Architecture

Contains parts

- agent — manages Xray/Wireguard(in progress) connections/users/metrics
- api — manages cluster of servers, gets API calls and send commands to servers
- auth — handles auth for Hysteri2 cleints
- utils — helper to work/debug Bin messages

### As dependencies the platfrom has

- ZeroMQ — communicating bus
- PostgreSQL — user and node data storage
- Clickhouse — metrics storage
- Carbon — metrics relay
- Xray Core
- Hysteria2
- MTproto
- Wireguard
- Nginx — reverse proxy

### Features

- Standalone Agent — can run without external dependencies: just WG or/and Xray Core and a config file.
- Automatic Xray Config Parsing — reads xray-config.json to fetch inbounds and settings automatically.
- Low Resource Usage — works perfectly on low-cost 1 CPU ($5 VPS) machines.
- Protocol Support — handles VLESS TCP, VLESS gRPC, VLESS Xhttp, Hysteria2 and Wireguard connections.
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
