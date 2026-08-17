# Canonical genesis migration

The checked-in `genesis.json` is a legacy, uncommitted test fixture. Hardened
nodes intentionally reject it. Do not launch a network by copying its validator
or allocation identifiers.

A launch manifest must provide all of the following:

- `protocolVersion: 2`, canonical network and chain identifiers, and a nonzero
  timestamp;
- `genesisValidatorPublicKey`, containing the exact lowercase hexadecimal
  ML-DSA-65 public key whose chain-scoped validator ID is
  `genesisValidator`;
- `accountPublicKey` on every allocation, with `address` derived from that key
  by the chain-scoped V2 account domain;
- strictly address-sorted, unique, nonzero allocations totaling exactly
  `21000000000000000000` base units for the contained fee-only V1 network;
- the sealed `QNTO`, 9-decimal, and maximum-supply fields; and
- no unknown fields or uncommitted `properties` maps.

Private keys never belong in this file. Keep validator and treasury private
keys in encrypted keystores backed up offline, and inject unlock credentials
through the deployment secret manager or environment variables—not committed
configuration.

Calculate the canonical commitments offline:

```sh
cargo run -p qanto --bin genesis_manifest -- config/genesis.json
```

Copy the returned `genesis_manifest_hash` into the node configuration as:

```toml
genesis_manifest_hash = "<64 lowercase hexadecimal characters>"
```

Every validator must independently reproduce the same manifest hash and
allocation state root before launch. Existing data directories without these
persisted commitments are deliberately rejected; start from a backed-up fresh
directory and replay the approved manifest. A later V2 emission reset requires
its own approved activation artifact and must not reuse this fee-only manifest.
