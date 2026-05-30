//! QANTO Libp2p Gossip Mesh
use libp2p::{identity, PeerId, SwarmBuilder};
use libp2p::mdns::{tokio::Behaviour as Mdns, Config as MdnsConfig, Event as MdnsEvent};
use std::error::Error;
use futures::StreamExt;

pub async fn initialize_p2p_mesh() -> Result<(), Box<dyn Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("🌐 QANTO Libp2p Mesh Initializing... Local Peer ID: {}", local_peer_id);
    
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
                _ => {}
            }
        }
    });

    Ok(())
}
