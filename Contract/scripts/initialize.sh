#!/bin/bash
set -euo pipefail

# Initialize script for Vaulty smart contracts
# Calls each contract's initialization entry point after deployment
# Requires administrator configuration for all contracts.

# Load configuration from environment variables
NETWORK="${NETWORK:-testnet}"
SOURCE_ACCOUNT="${SOURCE_ACCOUNT:-}"
CONTRACTS_DIR="${CONTRACTS_DIR:-target/wasm}"

# Administrator address is required for all contracts
ADMIN="${ADMIN:-}"
STREAKS_ADMIN="${STREAKS_ADMIN:-$ADMIN}"
LENDING_ADMIN="${LENDING_ADMIN:-$ADMIN}"
BORROWING_ADMIN="${BORROWING_ADMIN:-$ADMIN}"
REWARDS_ADMIN="${REWARDS_ADMIN:-$ADMIN}"

# Validate network
if [[ "$NETWORK" != "testnet" && "$NETWORK" != "mainnet" ]]; then
    echo "Error: NETWORK must be 'testnet' or 'mainnet'"
    exit 1
fi

# Check if source account is provided
if [ -z "$SOURCE_ACCOUNT" ]; then
    echo "Error: SOURCE_ACCOUNT environment variable must be set"
    exit 1
fi

# Check if admin is provided
if [ -z "$ADMIN" ]; then
    echo "Error: ADMIN environment variable must be set"
    echo "  ADMIN is the protocol administrator address required for all contracts"
    exit 1
fi

echo "Initializing Vaulty contracts on $NETWORK with admin $ADMIN..."

# Function to initialize a contract
initialize_contract() {
    local contract_name=$1
    local contract_id_file="$CONTRACTS_DIR/${contract_name}_id.txt"
    
    if [ ! -f "$contract_id_file" ]; then
        echo "Warning: Contract ID file not found for $contract_name, skipping"
        return
    fi
    
    local contract_id=$(cat "$contract_id_file")
    
    echo "Initializing $contract_name (ID: $contract_id)..."
    
    # Initialize based on contract type
    case "$contract_name" in
        vault)
            # Vault requires admin, streaks contract, and rewards contract
            if [ -z "$STREAKS_CONTRACT_ID" ] || [ -z "$REWARDS_CONTRACT_ID" ]; then
                echo "Error: STREAKS_CONTRACT_ID and REWARDS_CONTRACT_ID must be set for vault initialization"
                exit 1
            fi
            stellar contract invoke \
                --id "$contract_id" \
                --source "$SOURCE_ACCOUNT" \
                --network "$NETWORK" \
                -- initialize \
                --admin "$ADMIN" \
                --streaks_contract "$STREAKS_CONTRACT_ID" \
                --rewards_contract "$REWARDS_CONTRACT_ID"
            echo "Vault contract initialized with admin $ADMIN"
            ;;
        streaks)
            # Streaks requires a vault contract address
            if [ -z "$VAULT_CONTRACT_ID" ]; then
                echo "Error: VAULT_CONTRACT_ID must be set for streaks initialization"
                exit 1
            fi
            stellar contract invoke \
                --id "$contract_id" \
                --source "$SOURCE_ACCOUNT" \
                --network "$NETWORK" \
                -- initialize \
                --vault_contract "$VAULT_CONTRACT_ID"
            echo "Streaks contract initialized"
            ;;
        lending)
            # Lending requires admin for initialization
            stellar contract invoke \
                --id "$contract_id" \
                --source "$SOURCE_ACCOUNT" \
                --network "$NETWORK" \
                -- initialize \
                --admin "$LENDING_ADMIN"
            echo "Lending contract initialized with admin $LENDING_ADMIN"
            ;;
        borrowing)
            # Borrowing requires admin, lending pool, and vault contract
            if [ -z "$VAULT_CONTRACT_ID" ]; then
                echo "Error: VAULT_CONTRACT_ID must be set for borrowing initialization"
                exit 1
            fi
            stellar contract invoke \
                --id "$contract_id" \
                --source "$SOURCE_ACCOUNT" \
                --network "$NETWORK" \
                -- initialize \
                --admin "$BORROWING_ADMIN" \
                --lending_pool_address "$LENDING_CONTRACT_ID" \
                --vault_contract_address "$VAULT_CONTRACT_ID"
            echo "Borrowing contract initialized with admin $BORROWING_ADMIN"
            ;;
        rewards)
            # Rewards requires admin, reward asset, and streaks contract
            if [ -z "$STREAKS_CONTRACT_ID" ] || [ -z "$REWARD_ASSET" ]; then
                echo "Error: STREAKS_CONTRACT_ID and REWARD_ASSET must be set for rewards initialization"
                exit 1
            fi
            stellar contract invoke \
                --id "$contract_id" \
                --source "$SOURCE_ACCOUNT" \
                --network "$NETWORK" \
                -- initialize \
                --admin "$REWARDS_ADMIN" \
                --reward_asset "$REWARD_ASSET" \
                --streaks_contract "$STREAKS_CONTRACT_ID"
            echo "Rewards contract initialized with admin $REWARDS_ADMIN"
            ;;
        *)
            echo "Unknown contract: $contract_name"
            ;;
    esac
}

# Validate that all required contract IDs are available
if [ -z "${VAULT_CONTRACT_ID:-}" ]; then
    VAULT_CONTRACT_ID_FILE="$CONTRACTS_DIR/vault_id.txt"
    if [ -f "$VAULT_CONTRACT_ID_FILE" ]; then
        VAULT_CONTRACT_ID=$(cat "$VAULT_CONTRACT_ID_FILE")
    fi
fi

if [ -z "${STREAKS_CONTRACT_ID:-}" ]; then
    STREAKS_CONTRACT_ID_FILE="$CONTRACTS_DIR/streaks_id.txt"
    if [ -f "$STREAKS_CONTRACT_ID_FILE" ]; then
        STREAKS_CONTRACT_ID=$(cat "$STREAKS_CONTRACT_ID_FILE")
    fi
fi

if [ -z "${LENDING_CONTRACT_ID:-}" ]; then
    LENDING_CONTRACT_ID_FILE="$CONTRACTS_DIR/lending_id.txt"
    if [ -f "$LENDING_CONTRACT_ID_FILE" ]; then
        LENDING_CONTRACT_ID=$(cat "$LENDING_CONTRACT_ID_FILE")
    fi
fi

if [ -z "${REWARDS_CONTRACT_ID:-}" ]; then
    REWARDS_CONTRACT_ID_FILE="$CONTRACTS_DIR/rewards_id.txt"
    if [ -f "$REWARDS_CONTRACT_ID_FILE" ]; then
        REWARDS_CONTRACT_ID=$(cat "$REWARDS_CONTRACT_ID_FILE")
    fi
fi

# Initialize each contract
initialize_contract "vault"
initialize_contract "streaks"
initialize_contract "lending"
initialize_contract "borrowing"
initialize_contract "rewards"

echo "Initialization complete"
echo ""
echo "Admin configuration:"
echo "  Protocol admin: $ADMIN"
echo "  Streaks admin:  $STREAKS_ADMIN"
echo "  Lending admin:  $LENDING_ADMIN"
echo "  Borrowing admin: $BORROWING_ADMIN"
echo "  Rewards admin:  $REWARDS_ADMIN"
