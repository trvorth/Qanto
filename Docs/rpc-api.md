# QANTO RPC API Reference

## Overview

The QANTO RPC API allows developers to interact with the Layer-0 network programmatically. The primary endpoint is hosted on Hugging Face infrastructure for maximum uptime and global edge distribution.

> **Network specs:** 21,000,000,000 QNTO total supply · 1.0-second block time · 2.5 QNTO block reward

---

## Base URL

```
https://qanto-rpc.hf.space/api
```

All responses are returned as JSON with the following envelope:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { ... }
}
```

---

## Endpoints

### 1. Get Block Height

Returns the current block height of the chain.

**Request:**
```bash
curl -X POST https://qanto-rpc.hf.space/api \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain.getBlockHeight","params":[],"id":1}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "blockHeight": 1847293,
    "timestamp": "2026-04-19T07:30:00Z",
    "blockTime": "1.0s"
  }
}
```

---

### 2. Get Block by Height

Returns full block data for a given height.

**Request:**
```bash
curl -X POST https://qanto-rpc.hf.space/api \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"chain.getBlock","params":{"height":1847293},"id":1}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "blockHeight": 1847293,
    "hash": "0x9f3a...c4e1",
    "parentHash": "0x7b2d...a1f0",
    "timestamp": "2026-04-19T07:30:00Z",
    "txCount": 42,
    "minerAddress": "0x0000000000000000000000000000000000000001",
    "reward": "2.5 QNTO",
    "size": 12480
  }
}
```

---

### 3. Get Account Balance

Returns the QNTO balance for a given address.

**Request:**
```bash
curl -X POST https://qanto-rpc.hf.space/api \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"account.getBalance","params":{"address":"0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18"},"id":1}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18",
    "balance": "15000.000000000",
    "nonce": 47,
    "unit": "QNTO"
  }
}
```

---

### 4. Get Transaction by Hash

Returns transaction details for a given hash.

**Request:**
```bash
curl -X POST https://qanto-rpc.hf.space/api \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tx.getByHash","params":{"hash":"0xabc123..."},"id":1}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "hash": "0xabc123...",
    "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18",
    "to": "0x1234567890abcdef1234567890abcdef12345678",
    "value": "100.000000000 QNTO",
    "fee": "0.001000000 QNTO",
    "blockHeight": 1847290,
    "status": "confirmed",
    "timestamp": "2026-04-19T07:29:57Z"
  }
}
```

---

### 5. Get Network Status

Returns overall network health and chain parameters.

**Request:**
```bash
curl -X POST https://qanto-rpc.hf.space/api \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net.getStatus","params":[],"id":1}'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "chainId": "qanto-testnet-1",
    "blockHeight": 1847293,
    "blockTime": "1.0s",
    "totalSupply": "21000000000",
    "circulatingSupply": "4617733.250000000",
    "blockReward": "2.5",
    "peerCount": 128,
    "consensusMode": "hybrid-pow-dpos-pose",
    "pqcEnabled": true
  }
}
```

---

### 6. Submit Transaction

Broadcasts a signed transaction to the network.

**Request:**
```bash
curl -X POST https://qanto-rpc.hf.space/api \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "method":"tx.broadcast",
    "params":{
      "signedTx": "0xf86c808504a817c800825208..."
    },
    "id":1
  }'
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "txHash": "0xdef456...",
    "status": "pending",
    "estimatedConfirmation": "1.0s"
  }
}
```

---

## Error Codes

| Code   | Message                  | Description                                      |
| ------ | ------------------------ | ------------------------------------------------ |
| -32600 | Invalid Request          | Malformed JSON-RPC request.                      |
| -32601 | Method Not Found         | The requested method does not exist.             |
| -32602 | Invalid Params           | Missing or invalid parameters.                   |
| -32603 | Internal Error           | Server-side processing failure.                  |
| -32001 | Block Not Found          | The requested block height does not exist.       |
| -32002 | Account Not Found        | The requested address has no on-chain state.     |
| -32003 | Transaction Failed       | Transaction was rejected by the mempool.         |
| -32010 | Rate Limit Exceeded      | Too many requests — retry after cooldown.        |

---

## Rate Limits

| Tier        | Requests/min | Burst |
| ----------- | ------------ | ----- |
| **Free**    | 60           | 10    |
| **Builder** | 600          | 50    |
| **Node**    | Unlimited    | —     |

---

## JavaScript SDK Example

```javascript
async function getBlockHeight() {
  const response = await fetch('https://qanto-rpc.hf.space/api', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method: 'chain.getBlockHeight',
      params: [],
      id: 1
    })
  });
  const data = await response.json();
  return data.result.blockHeight;
}

async function getBalance(address) {
  const response = await fetch('https://qanto-rpc.hf.space/api', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      method: 'account.getBalance',
      params: { address },
      id: 1
    })
  });
  const data = await response.json();
  return data.result.balance;
}
```

---

*Last updated: April 2026 — QANTO v1.0.0*
