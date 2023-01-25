#! /bin/bash
set -e

create_deltafi_mint() {
  solana-keygen grind --starts-with tst:1 >/tmp/grind.txt
  cat /tmp/grind.txt
  token_address_keypair=$(cat /tmp/grind.txt | tail -1  | cut -d' ' -f 4)

  # Create token mint
  spl-token create-token --decimals 6 -- ${token_address_keypair} > /tmp/create-token.txt
  cat /tmp/create-token.txt
  token_address=$(cat /tmp/create-token.txt | head -1  | cut -d' ' -f 3)

  # Create token account and mint 1B tokens
  spl-token create-account ${token_address}
  spl-token mint ${token_address} 1000000000

  # Disable mint authority
  spl-token authorize ${token_address} mint --disable

  # Show current information
  spl-token account-info ${token_address}
  spl-token balance ${token_address}

  echo "Mint address is ${token_address}"
  echo ""
  echo "Mint authority is disabled, the following command should fail."
  echo "spl-token mint ${token_address} 1"
}

echo "The following key will be used to create token mint (~/.config/solana/id.json)."
solana-keygen pubkey

echo ""
echo "Do you wish to continue?"
select result in "Yes" "No"; do
  case $result in
    Yes ) create_deltafi_mint && exit;;
    No ) exit;;
  esac
done
