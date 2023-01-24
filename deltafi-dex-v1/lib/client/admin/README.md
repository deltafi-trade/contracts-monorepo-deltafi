# Run init script

### Download token config from solana github list
```
# edit the input list in token/mainnet-beta.input.json
node download_token_config.js
```

### Copy the keys to secret path in user home
```
mkdir -p ~/.deltafi/keys/dex-v1/
cp ./keys/dev-payer.json ~/.deltafi/keys/dex-v1/dev-payer.json
cp ./keys/dev-admin.json ~/.deltafi/keys/dex-v1/dev-admin.json
```

### localhost
```
node init_all.js localhost
```

### mainnet dev
```
node init_all.js
```

# Get config file for frontend

The get_frontend_config.js script parse the result of selected deployment category and prints out the JSON data

### testnet
```
node get_frontend_config.js testnet
```

### mainnet test
```
node get_frontend_config.js mainnet-test
```

### mainnet dev
```
node get_frontend_config.js mainnet-dev
```

### localhost
```
node get_frontend_config.js localhost
```
