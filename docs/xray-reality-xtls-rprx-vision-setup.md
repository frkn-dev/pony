

1. Add inbound 

```
{
  "tag": "Vless",
  "listen": "0.0.0.0",
  "port": 2053,
  "protocol": "vless",
  "settings": {
    "clients": [
    ],
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
      "privateKey": "PRIVATE_KEY", 
      "shortIds": [
        "SHORTID1",
        "SHORTID2"
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
```

2. Short_id 8-16 chars [a-f0-9] 

```openssl rand -hex 8```


3. Private_key

```xray x25519
Private key: SIQAE0jLxIGkaekrKn7kmLbORe_w8YKMrmuGiBmZRls
Public key: yhMUYkD9g0SfXB7htfXbbCpsBDGX3qyQkzyBX8a0VHk
```

4. Add user to Xray serverside 

uuid: ```xray uuid
dd2786a6-a174-4a2a-83c4-ba5085f5d835```

flow: xtls-rprx-vision

5. Client connection string (you should add user(uuid) first to Xray serverside)


```
vless://<uuid>@<server_ip>:<port>?security=reality&flow=xtls-rprx-vision&type=tcp&headerType=&serviceName=<service_name>&mode=gun&sni=<server_names>&fp=chrome&pbk=<public_key>&sid=<short_id>#<name in client>

example: vless://1ec1499c-c255-4d67-9d12-c5cd6c2a9a53@127.0.0.1:2053?security=reality&flow=xtls-rprx-vision&type=tcp&headerType=&serviceName=xyz&mode=gun&sni=discordapp.com&fp=chrome&pbk=hmjNjdJfVQQjzoxfrLrgsjdReONcDvGG8siXYEAAAAA&sid=e5c4d84fb339fb92#TEST-Vless-XTLS```

6. Optional: you can change SNI depends on your location
