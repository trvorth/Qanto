//! QANTO Libp2p Gossip Mesh
use futures::StreamExt;
use governor::{Quota, RateLimiter};
use libp2p::mdns::{tokio::Behaviour as Mdns, Config as MdnsConfig, Event as MdnsEvent};
use libp2p::{identity, PeerId, SwarmBuilder};
use std::error::Error;
use std::num::NonZeroU32;
use std::sync::Arc;

pub async fn initialize_p2p_mesh() -> Result<(), Box<dyn Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!(
        "🌐 QANTO Libp2p Mesh Initializing... Local Peer ID: {}",
        local_peer_id
    );

    let mdns = Mdns::new(MdnsConfig::default(), local_peer_id)?;
    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default().nodelay(true),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|_key| Ok(mdns))?
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    tokio::spawn(async move {
        // High-performance keyed rate limiter (50 TPS per peer) using the governor crate.
        // This provides the most optimized TokenBucket implementation without reinventing the wheel.
        let rate_limiter = Arc::new(RateLimiter::keyed(Quota::per_second(
            NonZeroU32::new(50).unwrap(),
        )));
        let _ = rate_limiter.check_key(&local_peer_id); // Type inference hint
        loop {
            match swarm.select_next_some().await {
                libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                    println!("🌐 P2P Mesh listening on {}", address);
                }
                libp2p::swarm::SwarmEvent::Behaviour(MdnsEvent::Discovered(peers)) => {
                    for (peer_id, addr) in peers {
                        println!("🌐 P2P Mesh discovered peer: {} at {}", peer_id, addr);
                        let _ = swarm.dial(addr);
                    }
                }
                libp2p::swarm::SwarmEvent::Behaviour(MdnsEvent::Expired(peers)) => {
                    for (peer_id, addr) in peers {
                        println!("🌐 P2P Mesh peer expired: {} at {}", peer_id, addr);
                    }
                }
                // Rate limiting check can be applied in message handling events:
                // if rate_limiter.check_key(&peer_id).is_ok() { /* process */ }
                _ => {}
            }
        }
    });

    Ok(())
}
