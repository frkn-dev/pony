

1. Add inbound 

```
 {
      "tag": "VlessGrpc",
      "listen": "0.0.0.0",
      "port": 2053,
      "protocol": "vless",
      "settings": {
        "clients": [],
        "decryption": "none"
      },
      "streamSettings": {
        "network": "grpc",
        "grpcSettings": {
          "serviceName": "xyz"
        },
        "security": "reality",
        "realitySettings": {
          "show": false,
          "dest": "discordapp.com:443",
          "xver": 0,
          "serverNames": [
            "cdn.discordapp.com",
            "discordapp.com"
          ],
          "privateKey": "4AQgu1qeCaGT8nnZTOnKLSOudSp_Z_AAAAAAAAAA",
          "shortIds": [
            "",
            "e5c4d84fb339fb92"
          ]
        }
      },
      "sniffing": {
        "enabled": true,
        "destOverride": [
          "http",
          "tls"
        ]
      }
    }


2. Client connection string (you should add user(uuid) first to Xray serverside)


```
vless://<uuid>@<server_ip>:<port>?security=reality&type=grpc&headerType=&serviceName=<service_name>&mode=gun&sni=<server_names>&fp=chrome&pbk=<public_key>&sid=<short_id>#<name in client>

example: vless://1ec1499c-c255-4d67-9d12-c5cd6c2a9a53@127.0.0.1:2053?security=reality&type=grpc&headerType=&serviceName=xyz&mode=gun&sni=cdn.discordapp.com&fp=chrome&pbk=hmjNjdJfVQQjzoxfrLrgsjdReONcDvGG8siXYEAAAAA&sid=e5c4d84fb339fb92#TEST-Vless-XTLS```
