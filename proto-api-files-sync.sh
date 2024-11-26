#!/bin/bash

### The script syncs specific API .proto files from Xray-core repo to dst dir ./src/proto

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <source_directory>"
  exit 1
fi

SOURCE_DIR="$1"
DEST_DIR="./src/proto"

if [ ! -d "$SOURCE_DIR" ]; then
  echo "Error: Source directory '$SOURCE_DIR' does not exist."
  exit 1
fi

FILES_TO_COPY=(
  "app/proxyman/command/command.proto"
  "app/proxyman/config.proto"
  "common/protocol/user.proto"
  "common/serial/typed_message.proto"
  "core/config.proto"
  "common/net/address.proto"
  "common/net/port.proto"
  "transport/internet/config.proto"
  "app/log/command/config.proto"
  "app/log/config.proto"
  "common/log/log.proto"
  "app/stats/command/command.proto"
  "app/stats/config.proto"
)

for file in "${FILES_TO_COPY[@]}"; do
  dest_file="$DEST_DIR/$file"
  mkdir -p "$(dirname "$dest_file")"
  cp "$SOURCE_DIR/$file" "$dest_file"
done

echo "Copying completed."
