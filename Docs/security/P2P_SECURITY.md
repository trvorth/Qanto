# P2P Security

**Qanto Layer-0 Blockchain**  
**Branch:** main

## Canonical P2P Stack

| Layer | Implementation |
|-------|---------------|
| Transport | libp2p (TCP, WebSocket) |
| Authentication | Noise XX |
| Multiplexing | Yamux |
| Peer Discovery | Kademlia DHT + configured initial peers |
| Message Routing | GossipSub |
| State Sync | QDS request-response |
| Peer Persistence | Atomic peer cache |

## Status: PulseMesh

**PulseMesh was never implemented.** Forensic audit found zero references to PulseMesh-specific components. No rollback required.

## Initial Peers

Three AWS nodes (Singapore, Frankfurt, Virginia) are **ordinary peers** with no protocol authority. Configured via `config.peers` as multiaddresses. Any peer can be an initial peer.

## Hardenings

| Capability | Status |
|------------|--------|
| Maximum frame size | Configurable, default 16MB |
| Maximum request/response size | Configurable per protocol |
| Per-peer connection limits | Connection manager |
| Timeouts | Request-level timeouts configured |
| Retry with jittered backoff | Implemented |
| Peer scoring | GossipSub scoring |
| Duplicate gossiop suppression | Implemented |
| Validate before propagation | Enabled |
| Atomic peer-cache writes | temp file + rename |
| Peer-cache corruption recovery | Corrupt file → fresh cache |
| DNS failure resilience | Peer cache covers initial peer unavailability |

## Test Matrix

| Test(Case | Status |
|-----------|--------|
| 3-node connection | Pending (E2E harness) |
| Single-surviving-peer join | Pending |
| Peer-cache restart | Pending |
| Malformed frames | Pending |
| Oversized frames | Pending |
| Invalid protocol versions | Pending |
| Forged peer IDs | Pending |