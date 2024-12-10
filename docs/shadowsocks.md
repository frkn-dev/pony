
### Shadowsocks


```inbound config
    
    {
      "tag": "Shadowsocks",
      "listen": "0.0.0.0",
      "port": 1080,
      "protocol": "shadowsocks",
      "settings": {
        "clients": [],
        "network": "tcp,udp"
      }
    }

```

```connection line

ss://<base64(chacha20-ietf-poly1305:password)>@<IP>:<Port>#Name-connection

example: ss://Y2hhY2hhMjAtaWV0Zi1wb2x5MTMwNTp0czlIV2l6QVRI@127.0.0.1:1080#Shadowsocks
```

```
Important - remove trailing end symbol

echo -n "chacha20-ietf-poly1305:ts9HWizATH" | base64
Y2hhY2hhMjAtaWV0Zi1wb2x5MTMwNTp0czlIV2l6QVRI

```
