use std::collections::HashMap;
use crate::shared::plugins::messaging::{check_messages_from_client, MessageReceivedFromPeer, MessageTrait, MessageTraitPlugin, SendArgs, ServerConnectionParams};
use bevy::app::App;
use bevy::asset::uuid::Uuid;
use bevy::ecs::system::SystemParam;
use bevy::prelude::{First, IntoScheduleConfigs, Last, MessageReader, Plugin, Resource, Time, Timer, TimerMode};
use message_pro_macro::ConnectionMessage;
use serde::{Deserialize, Serialize};
use crate::{NetRes, NetResMut};
use crate::server::plugins::network::PeersDroppedServer;

pub struct ServerPing;

pub struct PingPortsData {
    port_args: Option<SendArgs>
}

#[derive(Debug, Clone)]
pub struct PeerNetworkStats {
    pub rtt_ms: f32,
    pub smoothed_rtt_ms: f32,
    pub jitter_ms: f32,

    pub next_sequence: u32,
    pub pending_pings: HashMap<u32, f64>,
    pub timer: Timer
}

#[derive(Serialize, Deserialize, ConnectionMessage)]
pub struct PingMessage {
    pub sequence_id: u32,
    pub server_timestamp: f64,
    pub rtt_ms: f32,
}

#[derive(Serialize, Deserialize, ConnectionMessage)]
pub struct PongMessage {
    pub sequence_id: u32,
    pub server_timestamp: f64,
}

#[derive(Resource, Default)]
pub struct PingPorts(pub(crate) HashMap<u32, HashMap<u32,PingPortsData>>);

#[derive(Resource, Default)]
pub struct ServerNetworkStats(pub HashMap<Uuid, PeerNetworkStats>);

#[derive(SystemParam)]
#[allow(dead_code)]
pub struct PeerPings<'w> {
    time: NetRes<'w, Time>,
    server_network_stats: NetRes<'w, ServerNetworkStats>
}

#[allow(dead_code)]
impl <'w> PeerPings<'w> {
    fn get_ping(&self, _connection_id: u32, _port_id: u32, peer_uuid: &Uuid) -> f32 {
        if let Some(_peer_network_stats) = self.server_network_stats.0.get(peer_uuid) {

        }

        0.0
    }
}

impl PingPorts {
    pub fn add_ping_port(&mut self, connection_id: u32, port_id: u32, port_args: Option<SendArgs>) {
        if let Some(current_connections_list) = self.0.get_mut(&connection_id) {
            current_connections_list.insert(port_id, PingPortsData{
                port_args
            });
        }else {
            self.0.insert(connection_id, HashMap::from([(port_id, PingPortsData{ port_args })]));
        }
    }

    pub fn remove_ping_port(&mut self, connection_id: u32, port_id: u32){
        if let Some(current_connections_list) = self.0.get_mut(&connection_id) {
            current_connections_list.remove(&port_id);

            if current_connections_list.len() == 0 {
                self.0.remove(&connection_id);
            }
        }
    }
}

impl PeerNetworkStats {
    pub fn record_rtt(&mut self, new_rtt_ms: f32) {
        self.rtt_ms = new_rtt_ms;

        if self.smoothed_rtt_ms == 0.0 {
            self.smoothed_rtt_ms = new_rtt_ms;
            self.jitter_ms = 0.0;
        } else {
            let alpha = 0.1;

            let current_jitter = (new_rtt_ms - self.smoothed_rtt_ms).abs();
            self.jitter_ms = (alpha * current_jitter) + ((1.0 - alpha) * self.jitter_ms);

            self.smoothed_rtt_ms = (alpha * new_rtt_ms) + ((1.0 - alpha) * self.smoothed_rtt_ms);
        }
    }

    pub fn get_ping(&self) -> f32 {
        self.rtt_ms
    }

    pub fn get_smooth_ping(&self) -> f32 {
        self.smoothed_rtt_ms
    }

    pub fn get_smooth_ping_no_frame_delay(&self, time: &Time) -> f32 {
        let frame_time_ms = time.delta_secs() * 1000.0;

        (self.smoothed_rtt_ms - frame_time_ms).max(0.0)
    }
}

impl Plugin for ServerPing {
    fn build(&self, app: &mut App) {
        app.init_resource::<PingPorts>();
        app.init_resource::<ServerNetworkStats>();
        app.register_message::<PingMessage>();
        app.register_message::<PongMessage>();
        app.add_systems(First,process_pong.after(check_messages_from_client));
        app.add_systems(Last,(peer_disconnected,ping_ports).chain());
    }
}

impl Default for PeerNetworkStats {
    fn default() -> Self {
        Self {
            rtt_ms: 0.0,
            smoothed_rtt_ms: 0.0,
            jitter_ms: 0.0,
            next_sequence: 0,
            pending_pings: HashMap::new(),
            timer: Timer::from_seconds(1.0, TimerMode::Repeating)
        }
    }
}

fn ping_ports(
    ping_ports: NetRes<PingPorts>,
    mut server_connection_params: ServerConnectionParams,
    time: NetRes<Time>,
    mut stats: NetResMut<ServerNetworkStats>,
){
    let current_time = time.elapsed_secs_f64();

    for (connection_id,ports) in ping_ports.0.iter() {
        let mut keys_to_uses: Vec<Uuid> = Vec::new();

        if let Some(peers_authenticated) = server_connection_params.get_connections().get_peers_authenticated_immutable(*connection_id) {
            for uuid in peers_authenticated.keys() {
                if let Some(peer_network_stats) = stats.0.get_mut(uuid)
                {
                    if !peer_network_stats.timer.tick(time.delta()).just_finished() {
                        continue;
                    }

                    keys_to_uses.push(*uuid);
                }else {
                    let peer_network_stats = PeerNetworkStats::default();

                    stats.0.insert(*uuid, peer_network_stats);
                }
            }
        }

        for uuid in keys_to_uses.iter() {
            if let Some(peer_network_stats) = stats.0.get_mut(uuid) {
                let sequence_id = peer_network_stats.next_sequence;
                peer_network_stats.next_sequence = peer_network_stats.next_sequence.wrapping_add(1);

                peer_network_stats.pending_pings.insert(sequence_id, current_time);

                peer_network_stats.pending_pings.retain(|_, send_time| current_time - *send_time < 5.0);

                for (port_id,ping_ports_data) in ports.iter() {
                    server_connection_params.send_message(
                        *connection_id,
                        *port_id,
                        PingMessage {
                            sequence_id,
                            server_timestamp: current_time,
                            rtt_ms: peer_network_stats.smoothed_rtt_ms
                        },
                        *uuid,
                        ping_ports_data.port_args.as_ref()
                    );
                }
            }
        }
    }

}

fn process_pong(
    mut pong_message: MessageReader<MessageReceivedFromPeer<PongMessage>>,
    time: NetRes<Time>,
    mut stats: NetResMut<ServerNetworkStats>,
){
    let now = time.elapsed_secs_f64();

    for ev in pong_message.read() {
        let pong_message = &ev.message;

        if let Some(peer_network_stats) = stats.0.get_mut(&ev.peer_uuid)
        && let Some(send_time) = peer_network_stats.pending_pings.remove(&pong_message.sequence_id)
        {
            let rtt_seconds = now - send_time;
            let rtt_ms = (rtt_seconds * 1000.0) as f32;

            peer_network_stats.record_rtt(rtt_ms);
        }
    }
}

fn peer_disconnected(
    mut peers_dropped_server: MessageReader<PeersDroppedServer>,
    mut stats: NetResMut<ServerNetworkStats>
){
    for ev in peers_dropped_server.read() {
        for (_,(peer_uuid,_)) in ev.peers.iter() {
            if let Some(peer_uuid) = peer_uuid {
                stats.0.remove(peer_uuid);
            }
        }
    }
}