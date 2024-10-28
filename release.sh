#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 NEW_VERSION"
    exit 1
fi

NEW_VERSION="$1"
CARGO_TOML_PATH="Cargo.toml"

echo "Updating version in $CARGO_TOML_PATH to $NEW_VERSION..."
sed -i 's|^version = \".*\"|version = \"$NEW_VERSION\"|' "$CARGO_TOML_PATH"

sed 's|^version = \".*\"|version = \"$NEW_VERSION\"|' Cargo.toml

if [ $? -ne 0 ]; then
    echo "Failed to update version in $CARGO_TOML_PATH"
    exit 1
fi

git add "$CARGO_TOML_PATH"
git commit -m "Update version to $NEW_VERSION"

git tag "v$NEW_VERSION"

git push origin main
git push origin "v$NEW_VERSION"

echo "Version updated to $NEW_VERSION and changes pushed to the repository."
