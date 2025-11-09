#!/bin/bash

# Script to configure server URL and port for a Substrate node
# Usage: ./configure-url-port.sh <RPC_URL> <ACCOUNT> <SERVER_URL>

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check arguments
if [ "$#" -ne 3 ]; then
    echo -e "${RED}Error: Invalid number of arguments${NC}"
    echo "Usage: $0 <RPC_URL> <ACCOUNT> <SERVER_URL>"
    echo ""
    echo "Examples:"
    echo "  $0 ws://localhost:9944 //Alice localhost:3000        # Configure Alice's node"
    echo "  $0 ws://localhost:9945 //Bob localhost:3001          # Configure Bob's node"
    echo "  $0 wss://node.example.com:443 //Charlie 192.168.1.100:3002  # Configure remote node"
    exit 1
fi

RPC_URL=$1
ACCOUNT=$2
SERVER_URL=$3

# Convert full server URL to hex
SERVER_URL_HEX=$(echo -n "$SERVER_URL" | xxd -p | tr -d '\n')

echo -e "${YELLOW}Configuring node...${NC}"
echo "RPC URL: $RPC_URL"
echo "Account: $ACCOUNT"
echo "Server URL: $SERVER_URL (hex: 0x$SERVER_URL_HEX)"
echo ""

# Check if polkadot-js-api is installed
if ! command -v polkadot-js-api &> /dev/null; then
    echo -e "${YELLOW}polkadot-js-api not found. Please install it:${NC}"
    echo "npm install -g @polkadot/api-cli"
    echo ""
    echo -e "${YELLOW}Alternatively, use Polkadot.js Apps:${NC}"
    echo "1. Navigate to the Polkadot.js Apps interface for your node"
    echo "2. Go to Developer -> Extrinsics"
    echo "3. Select: template -> setServerConfig(server_url)"
    echo "4. Enter:"
    echo "   - server_url: 0x$SERVER_URL_HEX"
    echo "5. Submit with the desired account (e.g., Alice)"
    exit 1
fi

# Submit the transaction using polkadot-js-api
echo -e "${GREEN}Submitting transaction...${NC}"

polkadot-js-api \
    --ws "$RPC_URL" \
    --seed "$ACCOUNT" \
    tx.template.setServerConfig \
    "0x$SERVER_URL_HEX"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Successfully configured node at $RPC_URL${NC}"
    echo -e "${GREEN}  Server: http://$SERVER_URL${NC}"
else
    echo -e "${RED}✗ Failed to configure node${NC}"
    exit 1
fi
