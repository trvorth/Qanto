use crate::qanto_net::{
    CarbonOffsetCredential, NetworkMessage, PeerId, PeerInfo, QantoBlock, Transaction, UTXO,
};
use crate::qanto_serde::{
    QantoDeserialize, QantoDeserializer, QantoSerdeError, QantoSerialize, QantoSerializer,
};
// use std::collections::HashMap; // Removed unused import
use std::net::SocketAddr;
use std::time::{Duration, UNIX_EPOCH};

// --- Zero-Copy Serialization Implementations ---

impl QantoSerialize for PeerId {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_bytes(&self.id)
    }
}

impl QantoDeserialize for PeerId {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let bytes = deserializer.read_bytes(32)?;
        let mut id = [0u8; 32];
        id.copy_from_slice(&bytes);
        Ok(Self { id })
    }
}

impl QantoSerialize for QantoBlock {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_string(&self.id)?;
        QantoSerialize::serialize(&self.data, serializer)?;
        serializer.write_u32(self.chain_id)?;
        serializer.write_u64(self.height)?;
        serializer.write_u64(self.timestamp)?;
        QantoSerialize::serialize(&self.parents, serializer)?;
        serializer.write_u64(self.transactions_len as u64)?;
        Ok(())
    }
}

impl QantoDeserialize for QantoBlock {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let id = deserializer.read_string()?;
        let data = QantoDeserialize::deserialize(deserializer)?;
        let chain_id = deserializer.read_u32()?;
        let height = deserializer.read_u64()?;
        let timestamp = deserializer.read_u64()?;
        let parents = QantoDeserialize::deserialize(deserializer)?;
        let transactions_len = deserializer.read_u64()? as usize;
        Ok(Self {
            id,
            data,
            chain_id,
            height,
            timestamp,
            parents,
            transactions_len,
        })
    }
}

impl QantoSerialize for Transaction {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_string(&self.id)?;
        QantoSerialize::serialize(&self.data, serializer)?;
        Ok(())
    }
}

impl QantoDeserialize for Transaction {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let id = deserializer.read_string()?;
        let data = QantoDeserialize::deserialize(deserializer)?;
        Ok(Self { id, data })
    }
}

impl QantoSerialize for UTXO {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_string(&self.id)?;
        serializer.write_u64(self.value)?;
        Ok(())
    }
}

impl QantoDeserialize for UTXO {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let id = deserializer.read_string()?;
        let value = deserializer.read_u64()?;
        Ok(Self { id, value })
    }
}

impl QantoSerialize for CarbonOffsetCredential {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        serializer.write_string(&self.id)?;
        serializer.write_u64(self.amount)?;
        Ok(())
    }
}

impl QantoDeserialize for CarbonOffsetCredential {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let id = deserializer.read_string()?;
        let amount = deserializer.read_u64()?;
        Ok(Self { id, amount })
    }
}

impl QantoSerialize for PeerInfo {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        QantoSerialize::serialize(&self.peer_id, serializer)?;
        serializer.write_string(&self.address.to_string())?;
        QantoSerialize::serialize(&self.public_key, serializer)?;
        QantoSerialize::serialize(&self.capabilities, serializer)?;
        // SystemTime serialization - convert to duration since epoch
        let dur = self
            .last_seen
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        serializer.write_u64(dur.as_secs())?;
        serializer.write_u32(dur.subsec_nanos())?;

        serializer.write_i32(self.reputation)?;
        serializer.write_u32(self.connection_count)?;
        Ok(())
    }
}

impl QantoDeserialize for PeerInfo {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let peer_id = QantoDeserialize::deserialize(deserializer)?;
        let addr_str = deserializer.read_string()?;
        let address: SocketAddr = addr_str
            .parse()
            .map_err(|e| QantoSerdeError::Custom(format!("Invalid socket addr: {}", e)))?;
        let public_key = QantoDeserialize::deserialize(deserializer)?;
        let capabilities = QantoDeserialize::deserialize(deserializer)?;

        let secs = deserializer.read_u64()?;
        let nanos = deserializer.read_u32()?;
        let last_seen = UNIX_EPOCH + Duration::new(secs, nanos);

        let reputation = deserializer.read_i32()?;
        let connection_count = deserializer.read_u32()?;

        Ok(Self {
            peer_id,
            address,
            public_key,
            capabilities,
            last_seen,
            reputation,
            connection_count,
        })
    }
}

impl QantoSerialize for NetworkMessage {
    fn serialize<W: QantoSerializer>(&self, serializer: &mut W) -> Result<(), QantoSerdeError> {
        match self {
            NetworkMessage::Block(block) => {
                serializer.write_u8(0)?;
                QantoSerialize::serialize(block, serializer)?;
            }
            NetworkMessage::Transaction(tx) => {
                serializer.write_u8(1)?;
                QantoSerialize::serialize(tx, serializer)?;
            }
            NetworkMessage::BlockRequest { block_id } => {
                serializer.write_u8(2)?;
                serializer.write_string(block_id)?;
            }
            NetworkMessage::BlockResponse { block } => {
                serializer.write_u8(3)?;
                QantoSerialize::serialize(block, serializer)?;
            }
            NetworkMessage::StateRequest => {
                serializer.write_u8(4)?;
            }
            NetworkMessage::StateResponse { blocks, utxos } => {
                serializer.write_u8(5)?;
                QantoSerialize::serialize(blocks, serializer)?;
                QantoSerialize::serialize(utxos, serializer)?;
            }
            NetworkMessage::PeerDiscovery {
                peer_id,
                listen_addr,
                capabilities,
            } => {
                serializer.write_u8(6)?;
                QantoSerialize::serialize(peer_id, serializer)?;
                serializer.write_string(&listen_addr.to_string())?;
                QantoSerialize::serialize(capabilities, serializer)?;
            }
            NetworkMessage::PeerList(peers) => {
                serializer.write_u8(7)?;
                QantoSerialize::serialize(peers, serializer)?;
            }
            NetworkMessage::Ping { timestamp } => {
                serializer.write_u8(8)?;
                serializer.write_u64(*timestamp)?;
            }
            NetworkMessage::Pong { timestamp } => {
                serializer.write_u8(9)?;
                serializer.write_u64(*timestamp)?;
            }
            NetworkMessage::DhtFindNode { target } => {
                serializer.write_u8(10)?;
                QantoSerialize::serialize(target, serializer)?;
            }
            NetworkMessage::DhtFoundNodes { nodes } => {
                serializer.write_u8(11)?;
                QantoSerialize::serialize(nodes, serializer)?;
            }
            NetworkMessage::DhtStore { key, value } => {
                serializer.write_u8(12)?;
                QantoSerialize::serialize(key, serializer)?;
                QantoSerialize::serialize(value, serializer)?;
            }
            NetworkMessage::DhtFindValue { key } => {
                serializer.write_u8(13)?;
                QantoSerialize::serialize(key, serializer)?;
            }
            NetworkMessage::DhtValue { key, value } => {
                serializer.write_u8(14)?;
                QantoSerialize::serialize(key, serializer)?;
                QantoSerialize::serialize(value, serializer)?;
            }
            NetworkMessage::GossipMessage {
                topic,
                data,
                ttl,
                message_id,
            } => {
                serializer.write_u8(15)?;
                serializer.write_string(topic)?;
                QantoSerialize::serialize(data, serializer)?;
                serializer.write_u8(*ttl)?;
                serializer.write_bytes(message_id)?;
            }
            NetworkMessage::CarbonCredential(cred) => {
                serializer.write_u8(16)?;
                QantoSerialize::serialize(cred, serializer)?;
            }
            NetworkMessage::Consensus {
                round_id,
                block,
                shard_id,
            } => {
                serializer.write_u8(17)?;
                serializer.write_string(round_id)?;
                QantoSerialize::serialize(block, serializer)?;
                serializer.write_u64(*shard_id as u64)?;
            }
            NetworkMessage::Handshake {
                peer_id,
                public_key,
                signature,
                timestamp,
            } => {
                serializer.write_u8(18)?;
                QantoSerialize::serialize(peer_id, serializer)?;
                QantoSerialize::serialize(public_key, serializer)?;
                QantoSerialize::serialize(signature, serializer)?;
                serializer.write_u64(*timestamp)?;
            }
            NetworkMessage::HandshakeAck { peer_id, signature } => {
                serializer.write_u8(19)?;
                QantoSerialize::serialize(peer_id, serializer)?;
                QantoSerialize::serialize(signature, serializer)?;
            }
        }
        Ok(())
    }
}

impl QantoDeserialize for NetworkMessage {
    fn deserialize<R: QantoDeserializer>(deserializer: &mut R) -> Result<Self, QantoSerdeError> {
        let tag = deserializer.read_u8()?;
        match tag {
            0 => {
                let block = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::Block(block))
            }
            1 => {
                let tx = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::Transaction(tx))
            }
            2 => {
                let block_id = deserializer.read_string()?;
                Ok(NetworkMessage::BlockRequest { block_id })
            }
            3 => {
                let block = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::BlockResponse { block })
            }
            4 => Ok(NetworkMessage::StateRequest),
            5 => {
                let blocks = QantoDeserialize::deserialize(deserializer)?;
                let utxos = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::StateResponse { blocks, utxos })
            }
            6 => {
                let peer_id = QantoDeserialize::deserialize(deserializer)?;
                let addr_str = deserializer.read_string()?;
                let listen_addr: SocketAddr = addr_str
                    .parse()
                    .map_err(|e| QantoSerdeError::Custom(format!("Invalid socket addr: {}", e)))?;
                let capabilities = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::PeerDiscovery {
                    peer_id,
                    listen_addr,
                    capabilities,
                })
            }
            7 => {
                let peers = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::PeerList(peers))
            }
            8 => {
                let timestamp = deserializer.read_u64()?;
                Ok(NetworkMessage::Ping { timestamp })
            }
            9 => {
                let timestamp = deserializer.read_u64()?;
                Ok(NetworkMessage::Pong { timestamp })
            }
            10 => {
                let target = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::DhtFindNode { target })
            }
            11 => {
                let nodes = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::DhtFoundNodes { nodes })
            }
            12 => {
                let key = QantoDeserialize::deserialize(deserializer)?;
                let value = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::DhtStore { key, value })
            }
            13 => {
                let key = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::DhtFindValue { key })
            }
            14 => {
                let key = QantoDeserialize::deserialize(deserializer)?;
                let value = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::DhtValue { key, value })
            }
            15 => {
                let topic = deserializer.read_string()?;
                let data = QantoDeserialize::deserialize(deserializer)?;
                let ttl = deserializer.read_u8()?;
                let message_id_bytes = deserializer.read_bytes(32)?;
                let mut message_id = [0u8; 32];
                message_id.copy_from_slice(&message_id_bytes);
                Ok(NetworkMessage::GossipMessage {
                    topic,
                    data,
                    ttl,
                    message_id,
                })
            }
            16 => {
                let cred = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::CarbonCredential(cred))
            }
            17 => {
                let round_id = deserializer.read_string()?;
                let block = QantoDeserialize::deserialize(deserializer)?;
                let shard_id = deserializer.read_u64()? as usize;
                Ok(NetworkMessage::Consensus {
                    round_id,
                    block,
                    shard_id,
                })
            }
            18 => {
                let peer_id = QantoDeserialize::deserialize(deserializer)?;
                let public_key = QantoDeserialize::deserialize(deserializer)?;
                let signature = QantoDeserialize::deserialize(deserializer)?;
                let timestamp = deserializer.read_u64()?;
                Ok(NetworkMessage::Handshake {
                    peer_id,
                    public_key,
                    signature,
                    timestamp,
                })
            }
            19 => {
                let peer_id = QantoDeserialize::deserialize(deserializer)?;
                let signature = QantoDeserialize::deserialize(deserializer)?;
                Ok(NetworkMessage::HandshakeAck { peer_id, signature })
            }
            _ => Err(QantoSerdeError::InvalidTypeTag(tag)),
        }
    }
}
