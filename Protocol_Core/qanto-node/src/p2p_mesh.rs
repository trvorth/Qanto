//! QANTO Libp2p Gossip Mesh (Kademlia-based Discovery)
use futures::StreamExt;
use governor::{Quota, RateLimiter};
use libp2p::{
    identity, kad::{store::MemoryStore, Behaviour as KadBehaviour, Event as KadEvent}, 
    noise, PeerId, SwarmBuilder, swarm::{NetworkBehaviour, SwarmEvent}, 
    tcp, yamux
};
use std::error::Error;
use std::num::NonZeroU32;
use std::sync::Arc;

#[derive(NetworkBehaviour)]
struct MeshBehaviour {
    kademlia: KadBehaviour<MemoryStore>,
}

pub async fn initialize_p2p_mesh() -> Result<(), Box<dyn Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!(
        "🌐 QANTO Libp2p Mesh Initializing (Kademlia)... Local Peer ID: {}",
        local_peer_id
    );

    let store = MemoryStore::new(local_peer_id);
    let kademlia = KadBehaviour::new(local_peer_id, store);
    
    let behaviour = MeshBehaviour { kademlia };

    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_key| Ok(behaviour))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(std::time::Duration::from_secs(60)))
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Start Kademlia bootstrap
    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
        eprintln!("Failed to start Kademlia bootstrap: {e:?}");
    }

    tokio::spawn(async move {
        // High-performance keyed rate limiter (50 TPS per peer) using the governor crate.
        // This provides the most optimized TokenBucket implementation without reinventing the wheel.
        let rate_limiter = Arc::new(RateLimiter::keyed(Quota::per_second(
            NonZeroU32::new(50).unwrap(),
        )));
        let _ = rate_limiter.check_key(&local_peer_id); // Type inference hint
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("🌐 P2P Mesh listening on {}", address);
                }
                SwarmEvent::Behaviour(mesh_event) => match mesh_event {
                    MeshBehaviourEvent::Kademlia(kad_event) => match kad_event {
                        KadEvent::OutboundQueryProgressed { result, .. } => {
                            if let libp2p::kad::QueryResult::Bootstrap(Ok(_)) = result {
                                println!("🌐 P2P Mesh Kademlia bootstrap completed");
                            }
                        }
                        KadEvent::RoutingUpdated { peer, .. } => {
                            println!("🌐 P2P Mesh routing updated: {}", peer);
                        }
                        _ => {}
                    },
                },
                // Rate limiting check can be applied in message handling events:
                // if rate_limiter.check_key(&peer_id).is_ok() { /* process */ }
                _ => {}
            }
        }
    });

    Ok(())
}
