use newengine_replication_api::ReplicationReliability;
use serde::{Deserialize, Serialize};

pub const ENGINE_NETWORK_SERVICE_ID: &str = "engine.network";
pub const NETWORK_UDP_PROVIDER_ID: &str = "engine.network.udp";
pub const NETWORK_CONTRACT: &str = "newengine.network.v1";
pub const NETWORK_PACKET_WIRE_MAGIC: [u8; 4] = *b"NSN1";

pub mod network_method {
    pub const INFO_JSON_V1: &str = "network.info_json_v1";
    pub const BIND_JSON_V1: &str = "network.bind_json_v1";
    pub const CLOSE_JSON_V1: &str = "network.close_json_v1";
    pub const SEND_JSON_V1: &str = "network.send_json_v1";
    pub const SEND_REPLICATED_MESSAGE_JSON_V1: &str = "network.send_replicated_message_json_v1";
    pub const POLL_JSON_V1: &str = "network.poll_json_v1";
    pub const ENDPOINTS_JSON_V1: &str = "network.endpoints_json_v1";
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkBindRequestV1 {
    pub endpoint_id: String,
    pub bind_addr: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkBindResponseV1 {
    pub ok: bool,
    pub endpoint_id: String,
    pub local_addr: String,
    pub provider: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkCloseRequestV1 {
    pub endpoint_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSendRequestV1 {
    pub endpoint_id: String,
    pub peer_addr: String,
    pub channel: u8,
    pub reliability: ReplicationReliability,
    pub payload: Vec<u8>,
}

impl Default for NetworkSendRequestV1 {
    fn default() -> Self {
        Self {
            endpoint_id: String::new(),
            peer_addr: String::new(),
            channel: 0,
            reliability: ReplicationReliability::Unreliable,
            payload: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkReplicatedMessageSendRequestV1 {
    pub endpoint_id: String,
    pub peer_addr: String,
    pub message_id: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSendResponseV1 {
    pub ok: bool,
    pub endpoint_id: String,
    pub peer_addr: String,
    pub channel: u8,
    pub sequence: u64,
    pub bytes: usize,
    pub message_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkPollRequestV1 {
    pub endpoint_id: String,
    pub max_packets: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkPacketV1 {
    pub from_addr: String,
    pub channel: u8,
    pub reliability: ReplicationReliability,
    pub sequence: u64,
    pub payload: Vec<u8>,
}

impl Default for NetworkPacketV1 {
    fn default() -> Self {
        Self {
            from_addr: String::new(),
            channel: 0,
            reliability: ReplicationReliability::Unreliable,
            sequence: 0,
            payload: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkPollResponseV1 {
    pub endpoint_id: String,
    pub packets: Vec<NetworkPacketV1>,
    pub dropped_stale: u64,
    pub malformed: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkEndpointSnapshotV1 {
    pub endpoint_id: String,
    pub local_addr: String,
    pub sent_packets: u64,
    pub received_packets: u64,
    pub sent_bytes: u64,
    pub received_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkRuntimeSnapshotV1 {
    pub contract: String,
    pub provider: String,
    pub endpoints: Vec<NetworkEndpointSnapshotV1>,
}

pub fn udp_supports_reliability(reliability: ReplicationReliability) -> bool {
    matches!(
        reliability,
        ReplicationReliability::Unreliable | ReplicationReliability::UnreliableSequenced
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn udp_does_not_pretend_to_be_reliable() {
        assert!(udp_supports_reliability(ReplicationReliability::Unreliable));
        assert!(udp_supports_reliability(
            ReplicationReliability::UnreliableSequenced
        ));
        assert!(!udp_supports_reliability(ReplicationReliability::Reliable));
        assert!(!udp_supports_reliability(
            ReplicationReliability::ReliableOrdered
        ));
    }
}
