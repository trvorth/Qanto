# Private Engineering Testnet

This document describes the controlled private engineering network for Qanto protocol development and testing.

## Key Properties

- **Known operators only**: Access is restricted to authorized engineering personnel.
- **No monetary value**: Tokens on this network have no economic value.
- **No bridge**: Token transfers to other networks are disabled.
- **No sale**: There is no token sale or fundraising associated with this network.
- **No decentralization claim**: This network is not presented as decentralized or public.
- **Reset at any time**: The network state can be discarded and reinitialized without notice.
- **No history guarantee**: Historical data may be lost upon reset.
- **Fixed-difficulty containment**: Mining difficulty is fixed and does not adjust.
- **Not evidence of public readiness**: Success on this network does not indicate readiness for public testnet or mainnet.

## Operator Guidelines

Operators must:
1. Use only the private engineering configuration files.
2. Not assign monetary value to QNTO tokens on this network.
3. Not attempt to bridge tokens to other networks.
4. Not promote the network as having monetary value or decentralization properties.
5. Be prepared for frequent resets and state loss.
6. Keep operator keys and credentials secure and confidential.

## Configuration

The network uses the following configuration files:
- `config/readiness/private-engineering.toml`
- `config/readiness/private-engineering-genesis.json`
- `config/readiness/private-release-allowlist.toml`

These files define a resettable, non-value-bearing environment with fixed difficulty and emission disabled.

## Safety Notes

This network is strictly for engineering validation and protocol development. It does not test economic incentives, decentralization, or public network properties. Any appearance of value or success is coincidental and not indicative of real-world performance.