
### Vmess TCP 

```inbound config

{
      "tag": "Vmess",
      "listen": "0.0.0.0",
      "port": 8081,
      "protocol": "vmess",
      "settings": {
        "clients": []
      },
      "streamSettings": {
        "network": "tcp",
        "tcpSettings": {
          "header": {
            "type": "http",
            "request": {
              "method": "GET",
              "path": [
                "/"
              ],
              "headers": {
                "Host": [
                  "google.com"
                ]
              }
            },
            "response": {}
          }
        },
        "security": "none"
      }

 ````

 It's possible to change host on the client, it should be available in th country you are located



```connections string 

'{"add": "194.54.156.00", "aid": "0", "host": "google.com", "id": "dc79e5c9-4b10-48b3-b7b8-534821ceAAAA", "net": "tcp", "path": "/", "port": 8081, "ps": "VmessTCP Test", "scy": "auto", "tls": "none", "type": "http", "v": "2"}'

base64:

vmess://eyJhZGQiOiAiMTk0LjU0LjE1Ni4wMCIsICJhaWQiOiAiMCIsICJob3N0IjogImdvb2dsZS5jb20iLCAiaWQiOiAiZGM3OWU1YzktNGIxMC00OGIzLWI3YjgtNTM0ODIxY2VBQUFBIiwgIm5ldCI6ICJ0Y3AiLCAicGF0aCI6ICIvIiwgInBvcnQiOiA4MDgxLCAicHMiOiAiVm1lc3NUQ1AgVGVzdCIsICJzY3kiOiAiYXV0byIsICJ0bHMiOiAibm9uZSIsICJ0eXBlIjogImh0dHAiLCAidiI6ICIyIn0K

```
