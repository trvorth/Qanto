# Qanto Interchain Specification (Scaffold)

Version: 0.1.0

Scope:
- PacketV1 envelope with timeouts, sequence, and fee metadata
- Channel handshake (Init/Try/Ack/Confirm) and ordered/unordered semantics
- Router callbacks for modules (apps) and example ICS-20-like asset transfer
- ChainRegistry with governance-gated registration and pause/rotate flows
- ICS-23-like proofs over Keccak-based Merkle commitments and standardized encodings
- Qanto-specific light clients (DAG and Execution Chain) with misbehavior handling

Notes:
- This design is fully standalone and does not import Cosmos/Polkadot dependencies.
- Proof formats and client updates MUST be versioned and governed.
- Circuit breakers: per-channel caps, rate limiting, and auto-pause on repeated failures.

Next steps:
- Fill in Merkle proof verification and provide test vectors
- Implement light client verification (qanhash PoW, PQ signatures, finality)
- Complete channel handshake proof checks wired to ChainRegistry
- Implement AssetTransfer escrow/mint/burn logic and integrate with Router
- Provide a permissionless relayer with backpressure and fee handling

