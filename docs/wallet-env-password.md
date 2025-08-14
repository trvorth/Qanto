# Wallet Password Environment Variable Support

## Overview

The Qanto wallet system now supports reading wallet passwords from the `WALLET_PASSWORD` environment variable. This feature is particularly useful for:

- **Automated deployments**: No need for interactive password entry
- **CI/CD pipelines**: Seamless integration with automated testing and deployment
- **Docker containers**: Easy password injection via environment variables
- **Production environments**: Secure password management through secrets managers

## How It Works

All wallet-related binaries now check for the `WALLET_PASSWORD` environment variable before prompting for interactive input:

1. **First Priority**: Check `WALLET_PASSWORD` environment variable
2. **Fallback**: If not set, prompt for password interactively (for development)

## Supported Commands

The following commands support the `WALLET_PASSWORD` environment variable:

- `qantowallet` - All subcommands (generate, show, import, send, receive, balance)
- `qanto` - Node start and wallet generation
- `start_node` - Node startup with wallet
- `generate_wallet` - Standalone wallet generation
- `import_wallet` - Wallet import from mnemonic or private key

## Usage Examples

### Basic Usage

```bash
# Set the password in environment
export WALLET_PASSWORD="your_secure_password"

# Generate a new wallet (no prompt needed)
cargo run --bin generate_wallet

# Start a node (no prompt needed)
cargo run --bin qanto start

# Send a transaction (no prompt needed)
cargo run --bin qantowallet send --wallet wallet.key recipient_address 1000
```

### Docker Usage

```dockerfile
# In your Dockerfile
ENV WALLET_PASSWORD=${WALLET_PASSWORD}

# Or pass at runtime
docker run -e WALLET_PASSWORD="secure_password" your-qanto-image
```

### Kubernetes Secret Usage

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: wallet-password
type: Opaque
data:
  password: <base64-encoded-password>
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: qanto-node
spec:
  template:
    spec:
      containers:
      - name: qanto
        image: your-qanto-image
        env:
        - name: WALLET_PASSWORD
          valueFrom:
            secretKeyRef:
              name: wallet-password
              key: password
```

### AWS Secrets Manager Integration

```bash
# Retrieve password from AWS Secrets Manager
export WALLET_PASSWORD=$(aws secretsmanager get-secret-value \
  --secret-id qanto-wallet-password \
  --query SecretString --output text)

# Start the node
cargo run --bin qanto start
```

## Security Considerations

### Best Practices

1. **Never hardcode passwords**: Always use secure secret management systems
2. **Use strong passwords**: Generate cryptographically secure passwords
3. **Rotate regularly**: Change passwords periodically
4. **Limit exposure**: Set the environment variable only when needed
5. **Clean up**: Unset the variable after use in scripts

### Example Secure Script

```bash
#!/bin/bash
set -euo pipefail

# Retrieve password securely (example with pass)
WALLET_PASSWORD=$(pass show qanto/wallet-password)
export WALLET_PASSWORD

# Run your command
cargo run --bin qanto start

# Clean up
unset WALLET_PASSWORD
```

## Error Handling

The implementation includes robust error handling:

- **Empty password check**: Rejects empty passwords
- **Validation**: Ensures password meets minimum requirements
- **Clear error messages**: Provides helpful feedback when password is invalid

### Error Examples

```bash
# Empty password
export WALLET_PASSWORD=""
cargo run --bin qantowallet show
# Error: WALLET_PASSWORD environment variable is set but empty.

# No password set (falls back to prompt)
unset WALLET_PASSWORD
cargo run --bin qantowallet show
# Enter password: _
```

## Development vs Production

### Development Mode

For development, you can still use interactive password prompts:

```bash
# Don't set WALLET_PASSWORD
unset WALLET_PASSWORD

# Will prompt interactively
cargo run --bin qantowallet generate
```

### Production Mode

For production, use secure secret management:

```bash
# Example with HashiCorp Vault
export WALLET_PASSWORD=$(vault kv get -field=password secret/qanto/wallet)

# Run in production
./qanto start --config production.toml
```

## Testing

To test the environment variable functionality:

```bash
# Run the test script
./test_env_password.sh

# Or test manually
export WALLET_PASSWORD="test123"
cargo run --bin generate_wallet -- --output test.key
cargo run --bin qantowallet -- show --wallet test.key
```

## Troubleshooting

### Common Issues

1. **Password not recognized**
   - Ensure the environment variable is exported: `export WALLET_PASSWORD="..."`
   - Check for typos in the variable name

2. **Permission denied**
   - Ensure wallet file has correct permissions
   - Check file ownership

3. **Wrong password**
   - Verify the password matches the one used to create the wallet
   - Check for special characters that might need escaping

### Debug Mode

To see when environment variable is being used:

```bash
# The implementation logs when using env var
export WALLET_PASSWORD="password"
cargo run --bin qantowallet show
# Output: "Using password from WALLET_PASSWORD environment variable."
```

## Migration Guide

### For Existing Scripts

Update your automation scripts from:

```bash
# Old way (requires expect or similar)
echo "password" | cargo run --bin qanto start
```

To:

```bash
# New way (clean and simple)
export WALLET_PASSWORD="password"
cargo run --bin qanto start
```

### For Docker Deployments

Update your Docker Compose:

```yaml
# docker-compose.yml
services:
  qanto:
    image: qanto:latest
    environment:
      - WALLET_PASSWORD=${WALLET_PASSWORD}
    env_file:
      - .env  # Contains WALLET_PASSWORD=...
```

## Summary

The `WALLET_PASSWORD` environment variable support makes Qanto wallet operations more suitable for:

- Automated environments
- Cloud deployments
- CI/CD pipelines
- Container orchestration
- Production systems

While maintaining backward compatibility with interactive prompts for development use.
