#!/bin/bash

ANYBAR_HOST="127.0.0.1"
ANYBAR_PORT="1738"

send_to_anybar() {
    echo "$1" | nc -4u -w0 $ANYBAR_HOST $ANYBAR_PORT
}

echo "Building with debug feature..."
cargo build 

if [ $? -eq 0 ]; then
    send_to_anybar "green"  
    echo "Build successful!"
else
    send_to_anybar "red"  
    echo "Build failed!"
fi
