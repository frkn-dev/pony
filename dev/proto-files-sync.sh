#!/bin/bash

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <source_directory>"
  exit 1
fi

SOURCE_DIR="$1"
DEST_DIR="src/proto"

if [ ! -d "$SOURCE_DIR" ]; then
  echo "Error: Source directory '$SOURCE_DIR' does not exist."
  exit 1
fi

mkdir -p "$DEST_DIR"

find "$SOURCE_DIR" -type f -name "*.proto" | while read -r file; do
  relative_path="${file#$SOURCE_DIR/}"

  target_path="$DEST_DIR/$relative_path"

  mkdir -p "$(dirname "$target_path")"

  cp "$file" "$target_path"
done

echo "Copying completed."
