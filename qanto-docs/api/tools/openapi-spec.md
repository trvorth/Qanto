---
id: openapi-spec
title: OpenAPI Specification
sidebar_label: OpenAPI/Swagger Spec
---

# Qanto API OpenAPI Specification

This page provides the complete OpenAPI 3.0 specification for the Qanto Protocol REST API, enabling interactive testing and automatic code generation.

## Interactive API Explorer

<div class="highlight-box">

### 🔗 Try the API Live
Access our interactive API documentation at: **[api.qanto.org/docs](https://api.qanto.org/docs)**

- **Test endpoints** directly from your browser
- **Generate client code** in multiple programming languages
- **View real-time responses** from mainnet and testnet
- **Download** the complete OpenAPI specification

</div>

## OpenAPI Specification Download

### Full Specification Files

<div class="api-method api-method--get">GET</div>

**Download Links:**

- **[qanto-api-v2.yaml](https://api.qanto.org/openapi.yaml)** - Complete OpenAPI 3.0 specification
- **[qanto-api-v2.json](https://api.qanto.org/openapi.json)** - JSON format specification
- **[Postman Collection](https://api.qanto.org/postman-collection.json)** - Ready-to-import Postman collection

## Specification Overview

```yaml
openapi: 3.0.3
info:
  title: Qanto Protocol API
  description: |
    The Qanto Protocol API provides comprehensive access to the Qanto Layer-0 blockchain network.
    
    ## Features
    - Full transaction and block data access
    - Real-time WebSocket subscriptions
    - SAGA AI governance endpoints
    - Validator and network statistics
    - Post-quantum secure authentication
    
    ## Rate Limiting
    - Public endpoints: 100 requests/minute
    - Authenticated endpoints: 1000 requests/minute
    - Premium tier: 10,000 requests/minute
    
  version: 2.0.0
  contact:
    name: Qanto Protocol Support
    url: https://discord.gg/qanto
    email: support@qanto.org
  license:
    name: MIT
    url: https://opensource.org/licenses/MIT
servers:
  - url: https://api.qanto.org/v2
    description: Mainnet API Server
  - url: https://testnet-api.qanto.org/v2
    description: Testnet API Server
  - url: https://dev-api.qanto.org/v2
    description: Development API Server
```

## Authentication

The Qanto API supports multiple authentication methods:

### API Key Authentication

```yaml
components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key
      description: |
        API key for authenticated requests. Get your API key from:
        https://developer.qanto.org/api-keys
```

### JWT Bearer Token

```yaml
    BearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT
      description: |
        JWT token for user-authenticated requests. Obtain from:
        POST /auth/login
```

## Core Endpoints

### Account Operations

```yaml
paths:
  /accounts/{address}:
    get:
      summary: Get Account Information
      tags: [Accounts]
      parameters:
        - name: address
          in: path
          required: true
          schema:
            type: string
            pattern: '^qanto[0-9a-z]{39}$'
          example: 'qanto1abc123def456ghi789jkl012mno345pqr678stu'
      responses:
        '200':
          description: Account information retrieved successfully
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Account'
```

### Transaction Operations

```yaml
  /transactions:
    post:
      summary: Submit Transaction
      tags: [Transactions]
      security:
        - ApiKeyAuth: []
        - BearerAuth: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/TransactionRequest'
      responses:
        '201':
          description: Transaction submitted successfully
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/TransactionResponse'
```

### SAGA AI Endpoints

```yaml
  /saga/governance/proposals:
    get:
      summary: Get Governance Proposals
      tags: [SAGA AI]
      parameters:
        - name: status
          in: query
          schema:
            type: string
            enum: [pending, active, passed, rejected]
        - name: limit
          in: query
          schema:
            type: integer
            minimum: 1
            maximum: 100
            default: 20
      responses:
        '200':
          description: Governance proposals retrieved successfully
          content:
            application/json:
              schema:
                type: object
                properties:
                  proposals:
                    type: array
                    items:
                      $ref: '#/components/schemas/Proposal'
```

## Data Models

### Core Schemas

```yaml
components:
  schemas:
    Account:
      type: object
      properties:
        address:
          type: string
          description: The account address
          example: 'qanto1abc123def456ghi789jkl012mno345pqr678stu'
        balance:
          type: string
          description: Account balance in QNTO (smallest unit)
          example: '1000000000000'
        nonce:
          type: integer
          description: Transaction nonce for this account
          example: 42
        staked_amount:
          type: string
          description: Amount staked by this account
          example: '500000000000'
        validator_info:
          $ref: '#/components/schemas/ValidatorInfo'
          
    Transaction:
      type: object
      properties:
        hash:
          type: string
          description: Transaction hash
          example: '0xabcdef1234567890...'
        from:
          type: string
          description: Sender address
        to:
          type: string
          description: Recipient address
        amount:
          type: string
          description: Transfer amount
        fee:
          type: string
          description: Transaction fee
        timestamp:
          type: integer
          description: Unix timestamp
        status:
          type: string
          enum: [pending, confirmed, failed]
        block_height:
          type: integer
          description: Block height (if confirmed)
```

## Error Handling

```yaml
    Error:
      type: object
      properties:
        error:
          type: object
          properties:
            code:
              type: integer
              description: Error code
              example: 4001
            message:
              type: string
              description: Human-readable error message
              example: 'Invalid transaction signature'
            details:
              type: object
              description: Additional error context
            trace_id:
              type: string
              description: Request trace ID for debugging
              example: 'trace-123abc-456def'
```

## Code Generation

### Supported Languages

Generate client libraries automatically using the OpenAPI specification:

```bash
# Install OpenAPI Generator
npm install @openapitools/openapi-generator-cli -g

# Generate JavaScript client
openapi-generator-cli generate \
  -i https://api.qanto.org/openapi.yaml \
  -g javascript \
  -o ./qanto-js-client

# Generate Python client  
openapi-generator-cli generate \
  -i https://api.qanto.org/openapi.yaml \
  -g python \
  -o ./qanto-python-client

# Generate Go client
openapi-generator-cli generate \
  -i https://api.qanto.org/openapi.yaml \
  -g go \
  -o ./qanto-go-client
```

### Available Generators

| Language | Generator | Package Manager |
|----------|-----------|----------------|
| JavaScript | `javascript` | npm |
| TypeScript | `typescript-node` | npm |
| Python | `python` | pip |
| Go | `go` | go mod |
| Rust | `rust` | cargo |
| Java | `java` | maven |
| C# | `csharp` | nuget |
| PHP | `php` | composer |

## Testing Endpoints

### Healthcheck

Test API connectivity:

<div class="api-method api-method--get">GET</div> `/health`

```json
{
  "status": "ok",
  "version": "2.0.0",
  "network": "mainnet",
  "block_height": 1234567,
  "timestamp": 1640995200
}
```

### Network Info

<div class="api-method api-method--get">GET</div> `/network/info`

```json
{
  "chain_id": "qanto-mainnet-1",
  "network_version": "2.0.0",
  "consensus_version": "1.0.0",
  "total_validators": 150,
  "total_supply": "21000000000000000000",
  "staking_ratio": 0.67,
  "inflation_rate": 0.05
}
```

## Integration Examples

### cURL Examples

```bash
# Get account balance
curl -X GET "https://api.qanto.org/v2/accounts/qanto1abc123def456ghi789jkl012mno345pqr678stu" \
  -H "X-API-Key: your-api-key-here"

# Submit transaction
curl -X POST "https://api.qanto.org/v2/transactions" \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key-here" \
  -d '{
    "from": "qanto1sender...",
    "to": "qanto1recipient...",
    "amount": "1000000000",
    "fee": "1000000",
    "signature": "0xabcdef..."
  }'
```

### SDK Usage

```javascript
// JavaScript SDK
import { QantoClient } from '@qanto/sdk';

const client = new QantoClient({
  apiKey: 'your-api-key',
  network: 'mainnet'
});

const account = await client.accounts.get('qanto1abc...');
console.log('Balance:', account.balance);
```

## Support

- **API Documentation**: [api.qanto.org/docs](https://api.qanto.org/docs)
- **Developer Discord**: [discord.gg/qanto-dev](https://discord.gg/qanto-dev)
- **GitHub Issues**: [github.com/trvworth/qanto/issues](https://github.com/trvworth/qanto/issues)
- **Email Support**: [api-support@qanto.org](mailto:api-support@qanto.org)
