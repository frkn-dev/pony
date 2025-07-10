# Application Protocol Description

## Overview

The application communicates via ZeroMQ Pub/Sub pattern, exchanging binary messages serialized with rkyv on specific topics. Components subscribe to topics to receive messages about users and connections, and publish messages to control state changes.

This approach replaces plain JSON messaging with efficient zero-copy serialization, reducing message size and CPU overhead.
Supported Actions & Message Structures

Messages are sent as binary blobs representing the following Rust struct:

#[derive(Archive, Serialize, Deserialize, Clone, Debug)]
pub struct Message {
pub conn_id: UuidWrapper,
pub action: Action,
pub tag: ArchivedTagSimple,
pub wg: Option<WgParam>,
pub password: Option<String>,
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum Action {
Create,
Update,
Delete,
ResetStat,
}

CREATE/UPDATE/DELETE Connection

Connections represent individual tunnels (WireGuard, Shadowsocks, VLESS, etc.) and are managed similarly.

Binary message includes:

    conn_id: Connection UUID

    action: Create/Update/Delete

    tag: Protocol type string ("Wireguard", "Shadowsocks", "VlessXtls", etc.)

    wg: WireGuard parameters (keys, allowed IPs), if applicable

    password: Shadowsocks password, if applicable

Messaging Details

    Messages are serialized using rkyv for zero-copy performance.

    Each message is published on a ZeroMQ topic: usually the node ID, environment, or "all".

    Subscribers filter by topic and deserialize binary payloads to Message structs.

    The protocol supports efficient updates and state syncing between components.

### Examples in JSON

Create

    {
      "action": "create",
      "conn_id": "17865be5-e18b-40d6-b5af-e1c4d51ff50a",
      "tag": "Wireguard",
      "wg": {
        "keys": {
          "privkey": "LY8D/CyB/JT1uiFhK1yVKxBB3VMZeA0DzOAJEvgQw50=",
          "pubkey": "a4uH3iSdV6Ifc7thKL8IHTM8PkL/yPBUztN9xuoA2Do="
        },
        "address": {
          "ip": "10.10.0.24",
          "cidr": 32
        }
      },
      "password": null
    }

Update

    {
      "action": "update",
      "conn_id": "17865be5-e18b-40d6-b5af-e1c4d51ff50a",
      "tag": "Wireguard",
      "wg": {
        "keys": {
          "privkey": "NEW_PRIVATE_KEY_BASE64",
          "pubkey": "NEW_PUBLIC_KEY_BASE64"
        },
        "address": {
          "ip": "10.10.0.25",
          "cidr": 32
        }
      },
      "password": null
    }

Delete

    {
      "action": "delete",
      "conn_id": "17865be5-e18b-40d6-b5af-e1c4d51ff50a",
      "tag": "Wireguard",
      "wg": null,
      "password": null
    }
