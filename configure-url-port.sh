#!/bin/bash

# Script to configure server URL and port for a Substrate node
# Usage: ./configure-url-port.sh <RPC_PORT> <SERVER_URL> <SERVER_PORT>

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check arguments
if [ "$#" -ne 3 ]; then
    echo -e "${RED}Error: Invalid number of arguments${NC}"
    echo "Usage: $0 <RPC_PORT> <SERVER_URL> <SERVER_PORT>"
    echo ""
    echo "Example:"
    echo "  $0 9944 localhost 3000        # Configure Alice's node"
    echo "  $0 9945 localhost 3001        # Configure Bob's node"
    echo "  $0 9946 192.168.1.100 3002   # Configure Charlie's node"
    exit 1
fi

RPC_PORT=$1
SERVER_URL=$2
SERVER_PORT=$3

# Convert server URL to hex
SERVER_URL_HEX=$(echo -n "$SERVER_URL" | xxd -p | tr -d '\n')

echo -e "${YELLOW}Configuring node...${NC}"
echo "RPC Port: $RPC_PORT"
echo "Server URL: $SERVER_URL (hex: 0x$SERVER_URL_HEX)"
echo "Server Port: $SERVER_PORT"
echo ""

# Check if polkadot-js-api is installed
if ! command -v polkadot-js-api &> /dev/null; then
    echo -e "${YELLOW}polkadot-js-api not found. Please install it:${NC}"
    echo "npm install -g @polkadot/api-cli"
    echo ""
    echo -e "${YELLOW}Alternatively, use Polkadot.js Apps:${NC}"
    echo "1. Navigate to http://localhost:$RPC_PORT"
    echo "2. Go to Developer -> Extrinsics"
    echo "3. Select: sudo -> sudo(call)"
    echo "4. Select: template -> setServerConfig(server_url, server_port)"
    echo "5. Enter:"
    echo "   - server_url: 0x$SERVER_URL_HEX"
    echo "   - server_port: $SERVER_PORT"
    echo "6. Submit with Alice's account"
    exit 1
fi

# Submit the transaction using polkadot-js-api
echo -e "${GREEN}Submitting transaction...${NC}"

polkadot-js-api \
    --ws "ws://127.0.0.1:$RPC_PORT" \
    --seed "//Alice" \
    tx.sudo.sudo \
    "api.tx.template.setServerConfig('0x$SERVER_URL_HEX', $SERVER_PORT)" \
    --sudo

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Successfully configured node on port $RPC_PORT${NC}"
    echo -e "${GREEN}  Server: http://$SERVER_URL:$SERVER_PORT${NC}"
else
    echo -e "${RED}✗ Failed to configure node${NC}"
    exit 1
fi
