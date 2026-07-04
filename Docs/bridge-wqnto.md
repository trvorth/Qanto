# WQNTO Bridge Guide — Wrapping Native QNTO

## Overview

**WQNTO** (Wrapped QNTO) is the ERC-20 representation of native QNTO, designed for interoperability with decentralized exchanges (DEXs), DeFi protocols, and cross-chain bridges. WQNTO uses **6 decimal places** of precision, matching USDT/USDC conventions for seamless DEX trading pairs.

> **Key facts:**
> - Total native supply: **21,000,000,000 QNTO**
> - Block time: **1.0 second**
> - Block reward: **2.5 QNTO**
> - WQNTO decimals: **6**
> - Contract standard: **ERC-20**

---

## Why Wrap QNTO?

| Feature               | Native QNTO        | WQNTO (ERC-20)                |
| --------------------- | ------------------- | ------------------------------ |
| **Network**           | QANTO Layer-0       | Ethereum / EVM-compatible      |
| **Decimals**          | 18 (internal)       | 6 (trading precision)          |
| **DEX Compatible**    | ✗                   | ✓ (Uniswap, SushiSwap, etc.)  |
| **DeFi Composable**   | ✗                   | ✓ (lending, yield, LP)         |
| **CEX Listing Ready** | ✗                   | ✓ (standardized ERC-20)        |

---

## WQNTO Token Contract

```
Contract Address : 0x...WQNTO_CONTRACT_ADDRESS (TBD at launch)
Token Name       : Wrapped QNTO
Symbol           : WQNTO
Decimals         : 6
Standard         : ERC-20
```

### Contract Interface

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract WQNTO is ERC20, Ownable {
    uint8 private constant _DECIMALS = 6;

    constructor() ERC20("Wrapped QNTO", "WQNTO") Ownable(msg.sender) {}

    function decimals() public pure override returns (uint8) {
        return _DECIMALS;
    }

    /// @notice Mint WQNTO when native QNTO is deposited to the bridge
    function mint(address to, uint256 amount) external onlyOwner {
        _mint(to, amount);
    }

    /// @notice Burn WQNTO when native QNTO is released from the bridge
    function burn(address from, uint256 amount) external onlyOwner {
        _burn(from, amount);
    }
}
```

---

## How to Wrap (QNTO → WQNTO)

### Step 1: Connect to the QANTO Bridge

Navigate to the bridge interface at [bridge.qanto.org](https://bridge.qanto.org) or use the CLI:

```bash
qanto-node tx bridge wrap \
  --amount 1000000000 \
  --destination 0xYourEthAddress \
  --from validator
```

> **Conversion:** 1,000 native QNTO → 1,000.000000 WQNTO (6 decimals)

### Step 2: Wait for Finalization

- The bridge requires **12 block confirmations** on the QANTO side (~12 seconds at 1.0s block time).
- Once confirmed, the WQNTO is minted on the destination EVM chain.

### Step 3: Verify on EVM Explorer

Check your WQNTO balance on Etherscan or the relevant EVM explorer:

```
https://etherscan.io/token/WQNTO_CONTRACT_ADDRESS?a=0xYourEthAddress
```

---

## How to Unwrap (WQNTO → QNTO)

### Step 1: Approve & Burn

```javascript
// Using ethers.js
const wqnto = new ethers.Contract(WQNTO_ADDRESS, WQNTO_ABI, signer);

// Approve the bridge contract to spend your WQNTO
await wqnto.approve(BRIDGE_ADDRESS, ethers.parseUnits("1000", 6));

// Initiate the unwrap
const bridge = new ethers.Contract(BRIDGE_ADDRESS, BRIDGE_ABI, signer);
await bridge.unwrap(
  ethers.parseUnits("1000", 6),  // 1000.000000 WQNTO
  "qanto1xyz..."                  // Your native QANTO address
);
```

### Step 2: Receive Native QNTO

- The bridge burns WQNTO on the EVM side.
- After **12 block confirmations** on EVM, native QNTO is released to your QANTO address.

---

## Decimal Precision Reference

| Amount (Human) | Native QNTO (18 dec)          | WQNTO (6 dec)   |
| -------------- | ----------------------------- | ---------------- |
| 1 QNTO         | 1000000000000000000           | 1000000          |
| 0.5 QNTO       | 500000000000000000            | 500000           |
| 100 QNTO       | 100000000000000000000         | 100000000        |
| 2.5 QNTO       | 2500000000000000000           | 2500000          |

> **Important:** The block reward of **2.5 QNTO** maps to `2500000` in WQNTO's 6-decimal format.

---

## Adding WQNTO to MetaMask

1. Open MetaMask → **Import Token**
2. Enter the token contract address
3. Symbol will auto-fill as `WQNTO`
4. Decimals will auto-fill as `6`
5. Click **Add Custom Token** → **Import**

---

## DEX Trading Pairs

Once wrapped, WQNTO can be traded on any EVM-compatible DEX:

| Pair           | DEX                | Status     |
| -------------- | ------------------ | ---------- |
| WQNTO/USDT     | Uniswap V3         | Live       |
| WQNTO/ETH      | SushiSwap          | Live       |
| WQNTO/USDC     | Uniswap V3         | Planned    |

---

## Security Considerations

1. **Bridge Multisig:** The bridge contract is controlled by a 4-of-7 multisig with a 24-hour timelock.
2. **Audit Status:** The WQNTO contract has been audited by [Auditor TBD].
3. **Supply Cap:** WQNTO minting is capped by the native QNTO locked in the bridge — no over-minting is possible.
4. **Emergency Pause:** The bridge can be paused immediately by any 2 of the 7 signers.

---

*Last updated: April 2026 — QANTO v1.0.0*
