use std::{
    collections::BTreeMap,
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
    sync::Arc,
};

use newengine_network_api::*;
use newengine_replication_api::ReplicationReliability;
use newengine_replication_runtime::ReplicationDescriptorRegistry;
use parking_lot::Mutex;

const HEADER_LEN: usize = 14;
const MAX_DATAGRAM_BYTES: usize = 65_507;

#[derive(Clone)]
pub struct UdpNetworkRuntime {
    inner: Arc<Mutex<State>>,
    replication: ReplicationDescriptorRegistry,
}

struct State {
    endpoints: BTreeMap<String, Endpoint>,
}

struct Endpoint {
    socket: UdpSocket,
    local_addr: SocketAddr,
    next_sequence: BTreeMap<u8, u64>,
    last_received_sequence: BTreeMap<(SocketAddr, u8), u64>,
    sent_packets: u64,
    received_packets: u64,
    sent_bytes: u64,
    received_bytes: u64,
    dropped_stale: u64,
    malformed: u64,
}

impl UdpNetworkRuntime {
    pub fn new(replication: ReplicationDescriptorRegistry) -> Self {
        Self {
            inner: Arc::new(Mutex::new(State {
                endpoints: BTreeMap::new(),
            })),
            replication,
        }
    }

    pub fn bind(&self, request: NetworkBindRequestV1) -> Result<NetworkBindResponseV1, String> {
        let endpoint_id = request.endpoint_id.trim().to_owned();
        if endpoint_id.is_empty() {
            return Err("network endpoint_id must not be empty".to_owned());
        }
        let bind_addr = if request.bind_addr.trim().is_empty() {
            "127.0.0.1:0"
        } else {
            request.bind_addr.trim()
        };
        let socket = UdpSocket::bind(bind_addr).map_err(|error| {
            format!("bind UDP endpoint '{endpoint_id}' at '{bind_addr}': {error}")
        })?;
        socket
            .set_nonblocking(true)
            .map_err(|error| format!("set UDP endpoint '{endpoint_id}' nonblocking: {error}"))?;
        let local_addr = socket.local_addr().map_err(|error| error.to_string())?;
        let mut state = self.inner.lock();
        if state.endpoints.contains_key(&endpoint_id) {
            return Err(format!("network endpoint already exists: {endpoint_id}"));
        }
        state.endpoints.insert(
            endpoint_id.clone(),
            Endpoint {
                socket,
                local_addr,
                next_sequence: BTreeMap::new(),
                last_received_sequence: BTreeMap::new(),
                sent_packets: 0,
                received_packets: 0,
                sent_bytes: 0,
                received_bytes: 0,
                dropped_stale: 0,
                malformed: 0,
            },
        );
        Ok(NetworkBindResponseV1 {
            ok: true,
            endpoint_id,
            local_addr: local_addr.to_string(),
            provider: NETWORK_UDP_PROVIDER_ID.to_owned(),
        })
    }

    pub fn close(&self, endpoint_id: &str) -> bool {
        self.inner
            .lock()
            .endpoints
            .remove(endpoint_id.trim())
            .is_some()
    }

    pub fn send(&self, request: NetworkSendRequestV1) -> Result<NetworkSendResponseV1, String> {
        if !udp_supports_reliability(request.reliability) {
            return Err(format!(
                "UDP provider '{}' does not implement {:?}; install a reliability provider/layer instead",
                NETWORK_UDP_PROVIDER_ID, request.reliability
            ));
        }
        if request.payload.len() + HEADER_LEN > MAX_DATAGRAM_BYTES {
            return Err(format!(
                "network datagram exceeds UDP payload limit: {} bytes",
                request.payload.len()
            ));
        }
        let peer: SocketAddr =
            request.peer_addr.trim().parse().map_err(|error| {
                format!("invalid network peer '{}': {error}", request.peer_addr)
            })?;
        let mut state = self.inner.lock();
        let endpoint = state
            .endpoints
            .get_mut(request.endpoint_id.trim())
            .ok_or_else(|| format!("network endpoint not found: {}", request.endpoint_id))?;
        let next = endpoint.next_sequence.entry(request.channel).or_insert(0);
        *next = next.wrapping_add(1).max(1);
        let sequence = *next;
        let frame = encode_packet(
            request.channel,
            request.reliability,
            sequence,
            &request.payload,
        );
        let bytes = endpoint
            .socket
            .send_to(&frame, peer)
            .map_err(|error| format!("UDP send_to {peer}: {error}"))?;
        endpoint.sent_packets = endpoint.sent_packets.saturating_add(1);
        endpoint.sent_bytes = endpoint.sent_bytes.saturating_add(bytes as u64);
        Ok(NetworkSendResponseV1 {
            ok: true,
            endpoint_id: request.endpoint_id,
            peer_addr: peer.to_string(),
            channel: request.channel,
            sequence,
            bytes,
            message_id: None,
        })
    }

    pub fn send_replicated_message(
        &self,
        request: NetworkReplicatedMessageSendRequestV1,
    ) -> Result<NetworkSendResponseV1, String> {
        let message_id = request.message_id.trim();
        let descriptor = self
            .replication
            .snapshot()
            .messages
            .into_iter()
            .find(|descriptor| descriptor.message_id == message_id)
            .ok_or_else(|| format!("replicated message descriptor not registered: {message_id}"))?;
        let mut response = self.send(NetworkSendRequestV1 {
            endpoint_id: request.endpoint_id,
            peer_addr: request.peer_addr,
            channel: descriptor.channel,
            reliability: descriptor.reliability,
            payload: request.payload,
        })?;
        response.message_id = Some(message_id.to_owned());
        Ok(response)
    }

    pub fn poll(&self, request: NetworkPollRequestV1) -> Result<NetworkPollResponseV1, String> {
        let max_packets = request.max_packets.clamp(1, 4096);
        let mut state = self.inner.lock();
        let endpoint = state
            .endpoints
            .get_mut(request.endpoint_id.trim())
            .ok_or_else(|| format!("network endpoint not found: {}", request.endpoint_id))?;
        let mut packets = Vec::new();
        let mut buffer = vec![0u8; MAX_DATAGRAM_BYTES];
        while packets.len() < max_packets {
            let (bytes, from) = match endpoint.socket.recv_from(&mut buffer) {
                Ok(value) => value,
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => return Err(format!("UDP recv_from: {error}")),
            };
            endpoint.received_bytes = endpoint.received_bytes.saturating_add(bytes as u64);
            let Some((channel, reliability, sequence, payload)) = decode_packet(&buffer[..bytes])
            else {
                endpoint.malformed = endpoint.malformed.saturating_add(1);
                continue;
            };
            if reliability == ReplicationReliability::UnreliableSequenced {
                let last = endpoint
                    .last_received_sequence
                    .entry((from, channel))
                    .or_insert(0);
                if sequence <= *last {
                    endpoint.dropped_stale = endpoint.dropped_stale.saturating_add(1);
                    continue;
                }
                *last = sequence;
            }
            endpoint.received_packets = endpoint.received_packets.saturating_add(1);
            packets.push(NetworkPacketV1 {
                from_addr: from.to_string(),
                channel,
                reliability,
                sequence,
                payload: payload.to_vec(),
            });
        }
        Ok(NetworkPollResponseV1 {
            endpoint_id: request.endpoint_id,
            packets,
            dropped_stale: endpoint.dropped_stale,
            malformed: endpoint.malformed,
        })
    }

    pub fn snapshot(&self) -> NetworkRuntimeSnapshotV1 {
        let state = self.inner.lock();
        NetworkRuntimeSnapshotV1 {
            contract: NETWORK_CONTRACT.to_owned(),
            provider: NETWORK_UDP_PROVIDER_ID.to_owned(),
            endpoints: state
                .endpoints
                .iter()
                .map(|(id, endpoint)| NetworkEndpointSnapshotV1 {
                    endpoint_id: id.clone(),
                    local_addr: endpoint.local_addr.to_string(),
                    sent_packets: endpoint.sent_packets,
                    received_packets: endpoint.received_packets,
                    sent_bytes: endpoint.sent_bytes,
                    received_bytes: endpoint.received_bytes,
                })
                .collect(),
        }
    }
}

fn reliability_code(value: ReplicationReliability) -> Option<u8> {
    match value {
        ReplicationReliability::Unreliable => Some(0),
        ReplicationReliability::UnreliableSequenced => Some(1),
        ReplicationReliability::Reliable | ReplicationReliability::ReliableOrdered => None,
    }
}

fn reliability_from_code(value: u8) -> Option<ReplicationReliability> {
    match value {
        0 => Some(ReplicationReliability::Unreliable),
        1 => Some(ReplicationReliability::UnreliableSequenced),
        _ => None,
    }
}

fn encode_packet(
    channel: u8,
    reliability: ReplicationReliability,
    sequence: u64,
    payload: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&NETWORK_PACKET_WIRE_MAGIC);
    out.push(channel);
    out.push(reliability_code(reliability).unwrap_or(0));
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn decode_packet(bytes: &[u8]) -> Option<(u8, ReplicationReliability, u64, &[u8])> {
    if bytes.len() < HEADER_LEN || bytes[..4] != NETWORK_PACKET_WIRE_MAGIC {
        return None;
    }
    let channel = bytes[4];
    let reliability = reliability_from_code(bytes[5])?;
    let sequence = u64::from_le_bytes(bytes[6..14].try_into().ok()?);
    Some((channel, reliability, sequence, &bytes[HEADER_LEN..]))
}

#[derive(Clone)]
struct NetworkService {
    runtime: UdpNetworkRuntime,
}

impl newengine_plugin_api::ServiceV1 for NetworkService {
    fn id(&self) -> newengine_plugin_api::CapabilityId {
        abi_stable::std_types::RString::from(ENGINE_NETWORK_SERVICE_ID)
    }

    fn describe(&self) -> abi_stable::std_types::RString {
        use network_method as m;
        abi_stable::std_types::RString::from(serde_json::json!({
            "id": ENGINE_NETWORK_SERVICE_ID,
            "version": 1,
            "contract": NETWORK_CONTRACT,
            "protocol": "newengine.network/udp-v1",
            "provider": NETWORK_UDP_PROVIDER_ID,
            "methods": [m::INFO_JSON_V1,m::BIND_JSON_V1,m::CLOSE_JSON_V1,m::SEND_JSON_V1,m::SEND_REPLICATED_MESSAGE_JSON_V1,m::POLL_JSON_V1,m::ENDPOINTS_JSON_V1],
            "features": ["udp","nonblocking","unreliable","unreliable-sequenced","replication-message-descriptors"]
        }).to_string())
    }

    fn call(
        &self,
        method: newengine_plugin_api::MethodName,
        payload: newengine_plugin_api::Blob,
    ) -> abi_stable::std_types::RResult<newengine_plugin_api::Blob, abi_stable::std_types::RString>
    {
        use network_method as m;
        fn ok<T: serde::Serialize>(
            value: &T,
        ) -> abi_stable::std_types::RResult<
            newengine_plugin_api::Blob,
            abi_stable::std_types::RString,
        > {
            match serde_json::to_vec(value) {
                Ok(bytes) => {
                    abi_stable::std_types::RResult::ROk(newengine_plugin_api::Blob::from(bytes))
                }
                Err(error) => abi_stable::std_types::RResult::RErr(
                    abi_stable::std_types::RString::from(error.to_string()),
                ),
            }
        }
        fn decode<T: serde::de::DeserializeOwned>(
            payload: &newengine_plugin_api::Blob,
        ) -> Result<T, abi_stable::std_types::RString> {
            serde_json::from_slice(payload.as_slice())
                .map_err(|error| abi_stable::std_types::RString::from(error.to_string()))
        }
        fn err(
            error: String,
        ) -> abi_stable::std_types::RResult<
            newengine_plugin_api::Blob,
            abi_stable::std_types::RString,
        > {
            abi_stable::std_types::RResult::RErr(abi_stable::std_types::RString::from(error))
        }
        match method.as_str() {
            m::INFO_JSON_V1 | m::ENDPOINTS_JSON_V1 => ok(&self.runtime.snapshot()),
            m::BIND_JSON_V1 => match decode::<NetworkBindRequestV1>(&payload)
                .and_then(|request| self.runtime.bind(request).map_err(Into::into))
            {
                Ok(value) => ok(&value),
                Err(error) => abi_stable::std_types::RResult::RErr(error),
            },
            m::CLOSE_JSON_V1 => match decode::<NetworkCloseRequestV1>(&payload) {
                Ok(request) => ok(
                    &serde_json::json!({"ok": self.runtime.close(&request.endpoint_id), "endpoint_id": request.endpoint_id}),
                ),
                Err(error) => abi_stable::std_types::RResult::RErr(error),
            },
            m::SEND_JSON_V1 => match decode::<NetworkSendRequestV1>(&payload) {
                Ok(request) => match self.runtime.send(request) {
                    Ok(value) => ok(&value),
                    Err(error) => err(error),
                },
                Err(error) => abi_stable::std_types::RResult::RErr(error),
            },
            m::SEND_REPLICATED_MESSAGE_JSON_V1 => {
                match decode::<NetworkReplicatedMessageSendRequestV1>(&payload) {
                    Ok(request) => match self.runtime.send_replicated_message(request) {
                        Ok(value) => ok(&value),
                        Err(error) => err(error),
                    },
                    Err(error) => abi_stable::std_types::RResult::RErr(error),
                }
            }
            m::POLL_JSON_V1 => match decode::<NetworkPollRequestV1>(&payload) {
                Ok(request) => match self.runtime.poll(request) {
                    Ok(value) => ok(&value),
                    Err(error) => err(error),
                },
                Err(error) => abi_stable::std_types::RResult::RErr(error),
            },
            _ => err(format!("unknown network method '{}'", method.as_str())),
        }
    }
}

pub fn init_network_service(replication: ReplicationDescriptorRegistry) -> UdpNetworkRuntime {
    let runtime = UdpNetworkRuntime::new(replication);
    let service = newengine_plugin_api::ServiceV1Dyn::from_value(
        NetworkService {
            runtime: runtime.clone(),
        },
        abi_stable::sabi_trait::TD_Opaque,
    );
    let _ = newengine_plugin_host::host_register_service_impl(service);
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_replication_api::ReplicatedMessageDescriptor;
    use std::{thread, time::Duration};

    #[test]
    fn udp_loopback_sends_and_receives_real_packet() {
        let registry = ReplicationDescriptorRegistry::default();
        registry
            .register_message(ReplicatedMessageDescriptor {
                message_id: "game.test.ping".to_owned(),
                channel: 3,
                reliability: ReplicationReliability::UnreliableSequenced,
                max_rate_hz: 60,
                ..Default::default()
            })
            .unwrap();
        let runtime = UdpNetworkRuntime::new(registry);
        let a = runtime
            .bind(NetworkBindRequestV1 {
                endpoint_id: "a".into(),
                bind_addr: "127.0.0.1:0".into(),
            })
            .unwrap();
        let b = runtime
            .bind(NetworkBindRequestV1 {
                endpoint_id: "b".into(),
                bind_addr: "127.0.0.1:0".into(),
            })
            .unwrap();
        let sent = runtime
            .send_replicated_message(NetworkReplicatedMessageSendRequestV1 {
                endpoint_id: "a".into(),
                peer_addr: b.local_addr,
                message_id: "game.test.ping".into(),
                payload: b"hello".to_vec(),
            })
            .unwrap();
        assert_eq!(sent.channel, 3);
        let mut received = None;
        for _ in 0..50 {
            let polled = runtime
                .poll(NetworkPollRequestV1 {
                    endpoint_id: "b".into(),
                    max_packets: 8,
                })
                .unwrap();
            if let Some(packet) = polled.packets.into_iter().next() {
                received = Some(packet);
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        let packet = received.expect("loopback packet");
        assert_eq!(packet.payload, b"hello");
        assert_eq!(packet.channel, 3);
        assert_eq!(
            packet.reliability,
            ReplicationReliability::UnreliableSequenced
        );
        assert_eq!(a.provider, NETWORK_UDP_PROVIDER_ID);
    }

    #[test]
    fn udp_provider_rejects_reliable_descriptor_instead_of_lying() {
        let registry = ReplicationDescriptorRegistry::default();
        registry
            .register_message(ReplicatedMessageDescriptor {
                message_id: "game.test.reliable".to_owned(),
                reliability: ReplicationReliability::ReliableOrdered,
                ..Default::default()
            })
            .unwrap();
        let runtime = UdpNetworkRuntime::new(registry);
        let a = runtime
            .bind(NetworkBindRequestV1 {
                endpoint_id: "a".into(),
                bind_addr: "127.0.0.1:0".into(),
            })
            .unwrap();
        let b = runtime
            .bind(NetworkBindRequestV1 {
                endpoint_id: "b".into(),
                bind_addr: "127.0.0.1:0".into(),
            })
            .unwrap();
        let error = runtime
            .send_replicated_message(NetworkReplicatedMessageSendRequestV1 {
                endpoint_id: a.endpoint_id,
                peer_addr: b.local_addr,
                message_id: "game.test.reliable".into(),
                payload: vec![1, 2, 3],
            })
            .unwrap_err();
        assert!(error.contains("does not implement"));
    }
}
