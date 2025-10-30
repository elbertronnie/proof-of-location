#!/bin/bash

# Offchain Worker Key Setup Script
# This inserts the required key for the offchain worker to sign transactions

set -e

RPC_PORT="${RPC_PORT:-9944}"
NODE_URL="http://localhost:${RPC_PORT}"
KEY_TYPE="pof!"

# Check if node is running
echo "Checking if node is running at $NODE_URL..."
if ! curl -s --max-time 2 $NODE_URL > /dev/null 2>&1; then
    echo "❌ Error: Cannot connect to node at $NODE_URL"
    echo ""
    echo "Please make sure your Substrate node is running with:"
    echo "  cargo run --release -- --dev --rpc-port $RPC_PORT"
    echo ""
    echo "Or set the RPC_PORT environment variable:"
    echo "  RPC_PORT=9933 ./insert-key.sh"
    exit 1
fi

echo "✅ Node is running"
echo ""

# Get the account to use (default to Alice for development)
ACCOUNT="${1:-//Alice}"

# Alice's details for development
ALICE_SEED="bottom drive obey lake curtain smoke basket hold race lonely fit walk//Alice"
ALICE_PUBLIC_KEY="0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"

echo "=========================================="
echo "Inserting Offchain Worker Key"
echo "=========================================="
echo "Node URL:  $NODE_URL"
echo "Key Type:  $KEY_TYPE"
echo "Account:   $ACCOUNT"
echo "Public Key: $ALICE_PUBLIC_KEY"
echo "=========================================="
echo ""

# Insert the key with proper Alice seed and public key
RESPONSE=$(curl -s -H "Content-Type: application/json" \
  -d "{
    \"id\":1,
    \"jsonrpc\":\"2.0\",
    \"method\":\"author_insertKey\",
    \"params\":[
      \"$KEY_TYPE\",
      \"$ALICE_SEED\",
      \"$ALICE_PUBLIC_KEY\"
    ]
  }" \
  $NODE_URL)

echo "Response: $RESPONSE"
echo ""

# Check if successful
if echo "$RESPONSE" | grep -q '"result":null'; then
    echo "✅ Key inserted successfully!"
    echo ""
    echo "The offchain worker should now be able to sign transactions."
    echo "Watch the node logs for 'Offchain worker started at block'."
else
    echo "⚠️  Response received. Check if key was inserted."
    echo ""
    echo "If you see an error, possible issues:"
    echo "  1. Node might not support this RPC method"
    echo "  2. Key might already be inserted"
    echo "  3. Check node is running in dev mode (--dev)"
fi

echo ""
echo "=========================================="
echo "Additional Info"
echo "=========================================="
echo ""
echo "Available development accounts:"
echo "  //Alice"
echo "  //Bob"
echo "  //Charlie"
echo "  //Dave"
echo "  //Eve"
echo "  //Ferdie"
echo ""
echo "To use a different account:"
echo "  ./insert-key.sh //Bob"
echo ""
echo "To use a custom seed phrase:"
echo "  ./insert-key.sh \"your seed phrase here\""
echo ""
echo "=========================================="
