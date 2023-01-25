#!/usr/bin/env bash

set -e
cd "$(dirname "$0")/.."

source ./scripts/rust-version.sh stable
source ./scripts/solana-version.sh

export RUSTFLAGS="-D warnings"
export RUSTBACKTRACE=1

set -x

cargo +"$rust_stable" build-bpf

CLUSTER_URL=""
if [[ $1 == "localnet" ]]; then
    CLUSTER_URL="http://localhost:8899"
elif [[ $1 == "devnet" ]]; then
    CLUSTER_URL="https://api.devnet.solana.com"
elif [[ $1 == "testnet" ]]; then
    CLUSTER_URL="https://api.testnet.solana.com"
else
    echo "Unsupported network: $1"
    exit 1
fi

solana config set --url $CLUSTER_URL

keypair="$HOME"/.config/solana/id.json

if [ ! -f "$keypair" ]; then
    echo Generating keypair ...
    solana-keygen new -o "$keypair" --no-passphrase --silent
fi

solana config set --keypair ${keypair}

sleep 1

for i in {1..5}
do
    solana airdrop 1
done

solana deploy target/deploy/deltafi_swap.so tests/fixtures/deltafi-deploy.json

exit 0
