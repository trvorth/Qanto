# Qanto Relayer (scaffold)

A minimal, permissionless relayer concept for Qanto interchain channels.

Goals:
- Subscribe to events on source chain (new commitments)
- Fetch proofs and submit `RecvPacket` on destination
- Submit acknowledgements and handle timeouts
- Support fee metadata and backpressure

This is a placeholder; see `src/main.rs` for a simple CLI skeleton.

