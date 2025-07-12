# Application Protocol Design

## Overview

The application communicates via ZeroMQ Pub/Sub pattern, exchanging binary messages serialized with rkyv on specific topics. Components subscribe to topics to receive messages about users and connections, and publish messages to control state changes.

This approach replaces plain JSON messaging with efficient zero-copy serialization, reducing message size and CPU overhead.
Supported Actions & Message Structures

Check src/bin/utils.rs for debugging binary message sending

Messages are sent as binary blobs representing the following Rust struct:

    #[derive(Archive, Serialize, Deserialize, Clone, Debug)]
    pub struct Message {
        pub conn_id: UuidWrapper,
        pub action: Action,
        pub tag: ProtoTag,
        pub wg: Option<Param>,
        pub password: Option<String>,
    }

        #[derive(Archive, Serialize, Deserialize, Debug, Clone)]
    pub enum Action {
        Create,
        Update,
        Delete,
        ResetStat,
    }

    #[derive(
         Archive,
         Clone,
         Debug,
         RkyvDeserialize,
         RkyvSerialize,
         Deserialize,
         Serialize,
         PartialEq,
         Eq,
         Hash,
         Copy,
         ToSql,
         FromSql,
     )]
     #[archive_attr(derive(Clone, Debug))]
     #[archive(check_bytes)]
     #[postgres(name = "proto", rename_all = "snake_case")]
     pub enum ProtoTag {
         #[serde(rename = "VlessXtls")]
         VlessXtls,
         #[serde(rename = "VlessGrpc")]
         VlessGrpc,
         #[serde(rename = "Vmess")]
         Vmess,
         #[serde(rename = "Shadowsocks")]
         Shadowsocks,
         #[serde(rename = "Wireguard")]
         Wireguard,
     }

    #[derive(
        Archive, Clone, Debug, Serialize, Deserialize, RkyvDeserialize, RkyvSerialize, PartialEq,
    )]
    pub struct Param {
        pub keys: Keys,
        pub address: IpAddrMaskSerializable,
    }



    Note: password field only for Shadowsocks
    wg fileds only for wireguard

## CREATE/UPDATE/DELETE/ResetStat Connection

Connections represent individual tunnels (WireGuard, Shadowsocks, VLESS, etc.) and are managed similarly.

Binary message includes:

    conn_id: Connection UUID

    action: Create/Update/Delete/ResetStat

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

    Display output — binary message printed
    17865be5-e18b-40d6-b5af-e1c4d51ff50a | Create | Wireguard | privkey: LY8D/CyB/JT1uiFhK1yVKxBB3VMZeA0DzOAJEvgQw50= pubkey: a4uH3iSdV6Ifc7thKL8IHTM8PkL/yPBUztN9xuoA2Do= address: 10.10.0.24/32 | -

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

     Display output — binary message printed
    17865be5-e18b-40d6-b5af-e1c4d51ff50a | Update | Wireguard | privkey: NEW_PRIVATE_KEY_BASE64 pubkey: NEW_PUBLIC_KEY_BASE64 address: 10.10.0.25/32 | -

Delete

    {
      "action": "delete",
      "conn_id": "17865be5-e18b-40d6-b5af-e1c4d51ff50a",
      "tag": "Wireguard",
      "wg": null,
      "password": null
    }


    Display output — binary message printed
    17865be5-e18b-40d6-b5af-e1c4d51ff50a | Delete | Wireguard | - | -
