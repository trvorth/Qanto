# Qanto airdrop status

## Not available

Qanto does not currently publish a production or public-testnet airdrop. Do not connect a wallet, sign a claim, import a token address, or send funds based on older Qanto airdrop instructions.

The repository contains `SmartContracts/contracts/QantoDrop.sol`, an owner-administered Merkle-claim primitive. That source file is not evidence of a deployed campaign. At the time of the 2026-08-08 engineering audit:

- no verified production contract address or immutable campaign identifier was published;
- no audited allocation root or claimant dataset was published;
- the owner could replace the active root and withdraw funded tokens;
- instrumented Hardhat coverage for QantoDrop was 0%;
- the bridge proof/claim path was deliberately fail-closed;
- automatic protocol finality was disabled;
- QantoSwap was deliberately disabled;
- the TGE/vesting lifecycle was incomplete.

The canonical QNTO and WQNTO precision is **9 decimals**. No earlier 6-decimal example or placeholder address is valid.

## Requirements before a campaign

A real airdrop launch must publish and independently verify:

1. the network, chain ID, contract address, compiler settings, bytecode hash, and administrator controls;
2. the allocation policy, eligibility snapshot, canonical Merkle encoding, root, and reproducible root-generation tool;
3. claim deadline, root-update policy, unclaimed-token policy, pause/recovery process, and conflict-of-interest controls;
4. positive, negative, replay, multi-root, front-running, malicious-token, and property tests;
5. an independent contract audit and remediation re-test;
6. a public incident channel and a private vulnerability-reporting channel;
7. wallet/explorer endpoints proven against the actual deployment;
8. a signed launch announcement from established Qanto channels.

Until those gates pass, the only safe user action is **do not interact**.

## Scam warning

No legitimate operator needs a seed phrase or private key to determine eligibility. Treat unsolicited messages, placeholder addresses, “activation” payments, and urgent claim links as scams. Verify any future campaign against multiple official, independently authenticated sources before signing.
