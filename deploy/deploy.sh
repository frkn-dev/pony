#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 VERSION REMOTE_HOST [REMOTE_USER]"
    exit 1
fi

VERSION="$1"
REMOTE_HOST="$2"
REMOTE_USER="${3:-root}"

CONFIG_PATH="./servers/${REMOTE_HOST}.toml"   
SYSTEMD_PATH="./pony.service"               

scp "$CONFIG_PATH" "$SYSTEMD_PATH" "$REMOTE_USER@$REMOTE_HOST:/tmp/"

ssh "$REMOTE_USER@$REMOTE_HOST" /bin/bash << EOF
    set -e 

    mkdir -p /opt/pony
    cd /opt/pony 
    curl -L -o "pony" "https://github.com/frkn-dev/pony/releases/download/v${VERSION}/pony"  
    mv /tmp/${REMOTE_HOST}.toml /opt/pony/config.toml
    chmod +x /opt/pony/pony

    mv /tmp/pony.service /etc/systemd/system/pony.service
    systemctl daemon-reload
    systemctl enable pony.service
    systemctl restart pony.service
    systemctl status pony.service
EOF

echo "Pony version $VERSION has been deployed. Running on $REMOTE_HOST"
