#!/bin/bash

set -e 

# DO NOT EDIT 
# OVERRIDE settings with dot env file corresponding to the env/machine 

# Installation settings
XRAY_VERSION="${XRAY_VERSION:-v24.12.31}"
PONY_VERSION="${PONY_VERSION:-v0.1.1}"
INSTALL_DIR="/opt/vpn"
ARCH=$(uname -m)
XRAY_URL="https://github.com/XTLS/Xray-core/releases/download/$XRAY_VERSION/Xray-linux-64.zip"
AGENT_URL="https://github.com/frkn-dev/pony/releases/download/$PONY_VERSION/agent-$ARCH"
XRAY_CONFIG_PATH="$INSTALL_DIR/xray-config.json"
PONY_CONFIG_PATH="$INSTALL_DIR/config-agent.toml"

# Xray core settings 
VLESS_GRPC_INBOUND_ENABLED=${VLESS_GRPC_INBOUND_ENABLED:-true}
VLESS_XTLS_INBOUND_ENABLED=${VLESS_XTLS_INBOUND_ENABLED:-true}
VMESS_INBOUND_ENABLED=${VMESS_INBOUND_ENABLED:-true}

XRAY_ENABLED="${XRAY_ENABLED:-true}"
XRAY_API_PORT="${XRAY_API_PORT:-23456}"
SHADOWSOCKS_PORT="${SHADOWSOCKS_PORT:-1080}"
VMESS_HOST="${VMESS_HOST:-google.com}"
VMESS_PORT="${VMESS_PORT:-80}"
VLESS_XTLS_PORT="${VLESS_XTLS_PORT:-443}"
VLESS_GRPC_PORT="${VLESS_GRPC_PORT:-2053}"
VLESS_SERVER_NAME="discordapp.com"
VLESS_DEST="discordapp.com:443"
VLESS_SETTINGS_FILE="$INSTALL_DIR/vless.settings"

# WG Settings

WG_ENABLED="${WG_ENABLED:-true}"
WG_PORT="${WG_PORT:-51580}" 
WG_NETWORK="${WG_NETWORK:-10.0.0.0/16}"
WG_ADDRESS="${WG_ADDRESS:-10.0.0.1}"
WG_INTERFACE=${WG_INTERFACE:-wg0}
WG_DNS="${WG_DNS:-1.1.1.1 8.8.8.8}"

DNS=$(printf '"%s",' $WG_DNS)
DNS=${DNS%,}

if [ "$WG_ENABLED" = "true" ]; then
  if [ -z "$WG_PRIVKEY" ] || [ -z "$WG_PUBKEY" ]; then
    echo "❌ WG is enabled, but WG_PRIVKEY or WG_PUBKEY is not set!"
    exit 1
  fi
fi

# Pony agent settings 
CARBON_ADDRESS="${CARBON_ADDRESS:-localhost:2003}"
ZMQ_ENDPOINT="${ZMQ_ENDPOINT:-tcp://localhost:3000}"
ENV="${ENV:-dev}"
API_ENDPOINT=${API_ENDPOINT:-http://localhost:3005}
API_TOKEN=${API_TOKEN:-mysecrettoken}
LABEL="${LABEL:-🏴‍☠️ dev}"

echo alias dc="docker-compose" >> ~/.bashrc
echo alias s="systemctl" >> ~/.bashrc
echo alias j="journalctl" >> ~/.bashrc

source ~/.bashrc

mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

#apt-get update -y 
#apt-get install -y unzip wget curl net-tools uuid-runtime

# set up UUID
if [ -n "$UUID" ]; then
  FINAL_UUID="$UUID"
else
    FINAL_UUID=$(uuidgen | tr -d '\n')
fi

echo "Installing agent version $PONY_VERSION..."
echo "$AGENT_URL"
curl -L -o agent "$AGENT_URL"
chmod +x agent

cat <<EOF | tee /etc/systemd/system/agent.service
[Unit]
Description=Pony Agent Service
After=xray.service
Requires=xray.service

[Service]
Type=simple
ExecStart=$INSTALL_DIR/agent $PONY_CONFIG_PATH
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable agent

default_interface=$(ip route | grep '^default' | awk '{print $5}')
  
if [[ -n "$default_interface" ]]; then
  default_ip=$(ip -o -4 addr show dev "$default_interface" | awk '{print $4}' | cut -d'/' -f1 | head -n 1)
  echo "default_interface: $default_interface"
  echo "default_ip: $default_ip"
else
  echo "Cannot get default ip, abort"
  exit -1
fi

if [[ -f "$PONY_CONFIG_PATH" ]]; then
  echo "File $PONY_CONFIG_PATH already exist. skip."
else
    cat <<EOF | tee "$PONY_CONFIG_PATH"
[debug]
enabled = true
web_server = "127.0.0.1"
web_port = 3001

[carbon]
address = "$CARBON_ADDRESS"

[logging]
level = "debug"

[agent]
local = false
metrics_enabled = true
metrics_interval = 60
stat_enabled = true
stat_job_interval = 60
metrics_hb_interval = 1

[xray]
enabled = $XRAY_ENABLED
xray_config_path = "$XRAY_CONFIG_PATH"

[wg]
enabled = $WG_ENABLED
port = $WG_PORT
network= "$WG_NETWORK"
address = "$WG_ADDRES"
interface = "$WG_INTERFACE"
privkey = "$WG_PRIVKEY"
pubkey = "$WG_PUBKEY"
dns = [$DNS]


[zmq]
endpoint = "$ZMQ_ENDPOINT"

[node]
env = "$ENV"
uuid = "$FINAL_UUID"
hostname = "$HOSTNAME"
default_interface = "$default_interface"
ipv4 = "$default_ip"
label = "$LABEL"

[api]
endpoint = "$API_ENDPOINT"
token="$API_TOKEN"
EOF
fi

if [ "$WG_ENABLED" = "true" ]; then
  echo "Installing wireguard"
  #apt-get install -y wireguard wireguard-tools

  echo "Configuring Firewall (ufw)"

  ufw allow 51820/udp

  cat <<EOF | tee /etc/ufw/sysctl.conf > /dev/null 
#
# Configuration file for setting network variables. Please note these settings
# override /etc/sysctl.conf and /etc/sysctl.d. If you prefer to use
# /etc/sysctl.conf, please adjust IPT_SYSCTL in /etc/default/ufw. See
# Documentation/networking/ip-sysctl.txt in the kernel source code for more
# information.
#

# Uncomment this to allow this host to route packets between interfaces
net/ipv4/ip_forward=1
#net/ipv6/conf/default/forwarding=1
#net/ipv6/conf/all/forwarding=1

# Disable ICMP redirects. ICMP redirects are rarely used but can be used in
# MITM (man-in-the-middle) attacks. Disabling ICMP may disrupt legitimate
# traffic to those sites.
net/ipv4/conf/all/accept_redirects=0
net/ipv4/conf/default/accept_redirects=0
net/ipv6/conf/all/accept_redirects=0
net/ipv6/conf/default/accept_redirects=0

# Ignore bogus ICMP errors
net/ipv4/icmp_echo_ignore_broadcasts=1
net/ipv4/icmp_ignore_bogus_error_responses=1
net/ipv4/icmp_echo_ignore_all=0

# Don't log Martian Packets (impossible addresses)
# packets
net/ipv4/conf/all/log_martians=0
net/ipv4/conf/default/log_martians=0

#net/ipv4/tcp_fin_timeout=30
#net/ipv4/tcp_keepalive_intvl=1800

# Uncomment this to turn off ipv6 autoconfiguration
#net/ipv6/conf/default/autoconf=1
#net/ipv6/conf/all/autoconf=1

# Uncomment this to enable ipv6 privacy addressing
#net/ipv6/conf/default/use_tempaddr=2
#net/ipv6/conf/all/use_tempaddr=2
EOF
 
  cat <<EOF | tee /etc/ufw/before.rules > /dev/null

#
# rules.before
#
# Rules that should be run before the ufw command line added rules. Custom
# rules should be added to one of these chains:
#   ufw-before-input
#   ufw-before-output
#   ufw-before-forward
#

# Don't delete these required lines, otherwise there will be errors
*filter
:ufw-before-input - [0:0]
:ufw-before-output - [0:0]
:ufw-before-forward - [0:0]
:ufw-not-local - [0:0]
# End required lines


# allow all on loopback
-A ufw-before-input -i lo -j ACCEPT
-A ufw-before-output -o lo -j ACCEPT

# quickly process packets for which we already have a connection
-A ufw-before-input -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
-A ufw-before-output -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
-A ufw-before-forward -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT

# drop INVALID packets (logs these in loglevel medium and higher)
-A ufw-before-input -m conntrack --ctstate INVALID -j ufw-logging-deny
-A ufw-before-input -m conntrack --ctstate INVALID -j DROP

# ok icmp codes for INPUT
-A ufw-before-input -p icmp --icmp-type destination-unreachable -j ACCEPT
-A ufw-before-input -p icmp --icmp-type time-exceeded -j ACCEPT
-A ufw-before-input -p icmp --icmp-type parameter-problem -j ACCEPT
-A ufw-before-input -p icmp --icmp-type echo-request -j ACCEPT

# ok icmp code for FORWARD
-A ufw-before-forward -p icmp --icmp-type destination-unreachable -j ACCEPT
-A ufw-before-forward -p icmp --icmp-type time-exceeded -j ACCEPT
-A ufw-before-forward -p icmp --icmp-type parameter-problem -j ACCEPT
-A ufw-before-forward -p icmp --icmp-type echo-request -j ACCEPT

# allow dhcp client to work
-A ufw-before-input -p udp --sport 67 --dport 68 -j ACCEPT

#
# ufw-not-local
#
-A ufw-before-input -j ufw-not-local

# if LOCAL, RETURN
-A ufw-not-local -m addrtype --dst-type LOCAL -j RETURN

# if MULTICAST, RETURN
-A ufw-not-local -m addrtype --dst-type MULTICAST -j RETURN

# if BROADCAST, RETURN
-A ufw-not-local -m addrtype --dst-type BROADCAST -j RETURN

# all other non-local packets are dropped
-A ufw-not-local -m limit --limit 3/min --limit-burst 10 -j ufw-logging-deny
-A ufw-not-local -j DROP

# allow MULTICAST mDNS for service discovery (be sure the MULTICAST line above
# is uncommented)
-A ufw-before-input -p udp -d 224.0.0.251 --dport 5353 -j ACCEPT

# allow MULTICAST UPnP for service discovery (be sure the MULTICAST line above
# is uncommented)
-A ufw-before-input -p udp -d 239.255.255.250 --dport 1900 -j ACCEPT

# don't delete the 'COMMIT' line or these rules won't be processed
COMMIT

# Allow traffic from WireGuard interface
*nat
:POSTROUTING ACCEPT [0:0]
-A POSTROUTING -s $WG_NETWORK -o $default_interface -j MASQUERADE
COMMIT
EOF

  echo "Configuring Wireguard"
  mkdir -p /etc/wireguard
  cat <<EOF | tee /etc/wireguard/${WG_INTERFACE}.conf 
[Interface]
Address = $WG_ADDRES
ListenPort = $WG_PORT
PrivateKey = $WG_PRIVKEY
EOF

fi

if [ "$XRAY_ENABLED" = "true" ]; then

  echo "Installing xray-core version $XRAY_VERSION..."
  curl -L -o xray.zip "$XRAY_URL"
  unzip -o xray.zip -d "$INSTALL_DIR"
  rm xray.zip
  
  echo "XRAY Core Version"
  $INSTALL_DIR/xray --version
  
  cat <<EOF | tee /etc/systemd/system/xray.service
[Unit]
Description=Xray Core Service
After=network.target

[Service]
ExecStart=$INSTALL_DIR/xray -config $XRAY_CONFIG_PATH
Restart=on-failure

[Install]
WantedBy=multi-user.target  
EOF

  systemctl daemon-reload
  systemctl enable xray

  echo "Generating configurations..."
  
  if [[ ! -f "$VLESS_SETTINGS_FILE" ]]; then 
      $INSTALL_DIR/xray x25519 > $VLESS_SETTINGS_FILE
      echo "short_id: $(openssl rand -hex 6)" >> $VLESS_SETTINGS_FILE
  fi
  
  PRIVATE_KEY=$(cat $VLESS_SETTINGS_FILE | grep Private | awk '{print $3}')
  PUBLIC_KEY=$(cat $VLESS_SETTINGS_FILE | grep Public | awk '{print $3}')
  
  SHORT_ID=$(cat $VLESS_SETTINGS_FILE | grep short_id | awk '{print $2}')
  
  if [[ -f "$XRAY_CONFIG_PATH" ]]; then
    echo "File $XRAY_CONFIG_PATH already exist. skip."
  else
    inbounds=""

    if [ "$VMESS_INBOUND_ENABLED" = "true" ]; then
      ufw allow $VMESS_PORT
      inbounds="${inbounds}
      {
        \"tag\": \"Vmess\",
        \"listen\": \"0.0.0.0\",
        \"port\": ${VMESS_PORT},
        \"protocol\": \"vmess\",
        \"settings\": {
          \"clients\": []
        },
        \"streamSettings\": {
          \"network\": \"tcp\",
          \"tcpSettings\": {
            \"header\": {
              \"type\": \"http\",
              \"request\": {
                \"method\": \"GET\",
                \"path\": [ \"/\" ],
                \"headers\": {
                  \"Host\": [ \"$VMESS_HOST\" ]
                }
              },
              \"response\": {}
            }
          },
          \"security\": \"none\"
        },
        \"sniffing\": {
          \"enabled\": true,
          \"destOverride\": [ \"http\", \"tls\" ]
        }
      },"
    fi

    if [ "$VLESS_XTLS_INBOUND_ENABLED" = "true" ]; then
      ufw allow $VLESS_XTLS_PORT
      inbounds="${inbounds}
      {
        \"tag\": \"VlessXtls\",
        \"listen\": \"0.0.0.0\",
        \"port\": $VLESS_XTLS_PORT,
        \"protocol\": \"vless\",
        \"settings\": {
          \"clients\": [],
          \"decryption\": \"none\"
        },
        \"streamSettings\": {
          \"network\": \"tcp\",
          \"security\": \"reality\",
          \"realitySettings\": {
            \"show\": false,
            \"xver\": 0,
            \"serverNames\": [ \"$VLESS_SERVER_NAME\" ],
            \"privateKey\": \"$PRIVATE_KEY\",
            \"publicKey\": \"$PUBLIC_KEY\",
            \"shortIds\": [ \"$SHORT_ID\" ],
            \"dest\": \"$VLESS_DEST\"
          }
        },
        \"sniffing\": {
          \"enabled\": true,
          \"destOverride\": [ \"http\", \"tls\" ]
        }
      },"
    fi

    if [ "$VLESS_GRPC_INBOUND_ENABLED" = "true" ]; then
      ufw allow $VLESS_GRPC_PORT
      inbounds="${inbounds}
      {
        \"tag\": \"VlessGrpc\",
        \"listen\": \"0.0.0.0\",
        \"port\": $VLESS_GRPC_PORT,
        \"protocol\": \"vless\",
        \"settings\": {
          \"clients\": [],
          \"decryption\": \"none\"
        },
        \"streamSettings\": {
          \"network\": \"grpc\",
          \"grpcSettings\": {
            \"serviceName\": \"xyz\"
          },
          \"security\": \"reality\",
          \"realitySettings\": {
            \"show\": false,
            \"dest\": \"$VLESS_DEST\",
            \"xver\": 0,
            \"serverNames\": [ \"$VLESS_SERVER_NAME\" ],
            \"privateKey\": \"$PRIVATE_KEY\",
            \"publicKey\": \"$PUBLIC_KEY\",
            \"shortIds\": [ \"\", \"$SHORT_ID\" ]
          }
        },
        \"sniffing\": {
          \"enabled\": true,
          \"destOverride\": [ \"http\", \"tls\" ]
        }
      },"
    fi

    inbounds=$(echo "$inbounds" | sed '$s/},/}/')

  cat <<EOF | tee "$XRAY_CONFIG_PATH"
{
  "log": {
    "loglevel": "debug"
  },
  "inbounds": [
$inbounds
  ],
  "outbounds": [
    { "protocol": "freedom", "tag": "DIRECT" },
    { "protocol": "blackhole", "tag": "BLOCK" }
  ],
  "routing": {
    "rules": [
      {
        "inboundTag": ["API_INBOUND"],
        "source": ["127.0.0.1", "$default_ip"],
        "outboundTag": "API",
        "type": "field"
      },
      {
        "ip": ["geoip:private"],
        "outboundTag": "BLOCK",
        "type": "field"
      },
      {
        "domain": ["geosite:private"],
        "outboundTag": "BLOCK",
        "type": "field"
      },
      {
        "protocol": ["bittorrent"],
        "outboundTag": "BLOCK",
        "type": "field"
      }
    ]
  },
  "api": {
    "listen": "127.0.0.1:$XRAY_API_PORT",
    "services": [
      "HandlerService",
      "StatsService",
      "LoggerService",
      "ReflectionService"
    ],
    "tag": "API"
  },
  "stats": {},
  "policy": {
    "levels": {
      "0": {
        "statsUserUplink": true,
        "statsUserDownlink": true,
        "statsUserOnline": true
      }
    },
    "system": {
      "statsInboundDownlink": true,
      "statsInboundUplink": true,
      "statsOutboundDownlink": true,
      "statsOutboundUplink": true
    }
  }
}
EOF
  fi
fi

ufw allow 22

yes | ufw disable
yes | ufw enable
  

echo "Installation complete. Use the following commands to start services:"
echo "  sudo systemctl start xray"
echo "  sudo wg-quick up $WG_INTERFACE"
echo "  sudo ip route add $WG_NETWORK dev $WG_INTERFACE"
echo "  sudo systemctl start agent"
