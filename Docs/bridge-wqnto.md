# WQNTO integration status

## Launch status

**WQNTO is not a production cross-chain bridge.** The repository contains an ERC-20 wrapper contract, but the Qanto bridge verifier and native claim path are intentionally fail-closed pending a real, versioned proof system and independently reviewed bridge design. No canonical WQNTO deployment address, bridge multisig, confirmation policy, liquidity pool, or audit certificate is published by this repository.

Do not accept deposits, advertise live liquidity, or represent bridge claims as enabled until the bridge launch gates in `Docs/audits/2026-08-08-master-a-z/` are closed.

## Canonical token precision

Qanto and WQNTO use **9 decimal places**:

- `1 QNTO = 1_000_000_000` atomic units.
- `WQNTO.decimals()` returns `9`.
- `1.5 QNTO = 1_500_000_000` atomic units.

The 9-decimal value is defined by the Rust protocol constants and `SmartContracts/contracts/WQNTO.sol`. Integrations must query `decimals()` instead of assuming 6 or 18 decimals.

## What the current contract does

`SmartContracts/contracts/WQNTO.sol` implements a same-chain native-asset wrapper:

1. `deposit()` or a direct native transfer mints WQNTO to the sender one-for-one in atomic units.
2. `withdraw(amount)` burns the sender's WQNTO and returns the same amount of native currency.
3. The constructor takes no arguments.
4. There is no owner mint function, bridge validator set, remote-chain light client, or cross-chain finality rule in this contract.

Those semantics are similar to a conventional wrapped-native-token contract. They do **not** prove that native QNTO is locked on a different chain and do not make the contract a cross-chain bridge by themselves.

## Local verification

From `SmartContracts/`:

```bash
npm ci
npm test
npx hardhat compile
```

The deployment script loads the compiler-generated ABI and bytecode from `artifacts/contracts/WQNTO.sol/WQNTO.json`, verifies the zero-argument constructor, requires `RPC_URL` and `PRIVATE_KEY`, and refuses an unexpected chain ID. It does not embed a deployment key or a production address.

Example for an explicitly authorized environment:

```bash
RPC_URL=https://your-authorized-rpc \
PRIVATE_KEY=0x... \
EXPECTED_CHAIN_ID=1234 \
node deploy_wqnto.js
```

Use a secret manager or an ephemeral environment injection mechanism for deployment credentials. Never commit a private key or deployment `.env` file.

## Production gates

Before publishing a WQNTO bridge:

- approve and implement the versioned bridge protocol proposal;
- bind every accepted proof to the source chain, finalized block, event, token, amount, recipient, and replay domain;
- replace synthetic verifier logic with stable verification keys and real proof verification;
- define source and destination finality policies and reorganization handling;
- add replay protection and a supply invariant across locked native assets and minted wrapped assets;
- complete independent smart-contract and protocol audits;
- publish verified contract addresses, bytecode hashes, administrator controls, and an incident runbook;
- run adversarial, property, invariant, and cross-chain reorganization tests.

Until all gates pass, the safe operational state is **bridge disabled**.
