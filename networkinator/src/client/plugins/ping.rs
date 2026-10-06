use std::collections::HashMap;
use crate::shared::plugins::messaging::{check_messages_from_server, ClientConnectionParams, MessageReceivedFromServer, MessageTrait, MessageTraitPlugin, SendArgs};
use bevy::app::{App, PreUpdate};
use bevy::ecs::system::SystemParam;
use bevy::prelude::{First, IntoScheduleConfigs, MessageReader, Plugin, Resource, Time};
use message_pro_macro::ConnectionMessage;
use serde::{Deserialize, Serialize};
use crate::{NetRes, NetResMut};
use crate::client::plugins::network::ClientPortDisconnected;
use crate::shared::plugins::network::{ConnectionClosedClient, LocalSessionUUID};

pub struct ClientPing;

pub struct PingPortsData {
    port_args: Option<SendArgs>
}

pub struct PingsValues {
    ping_ms: f32
}

pub struct ServerTimeData {
    pub(crate) offset_secs: f64,
    pub(crate) is_synced: bool
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
pub struct ServerTime(pub(crate) HashMap<u32, HashMap<u32,ServerTimeData>>);

#[derive(Resource, Default)]
pub struct Pings(pub(crate) HashMap<u32, HashMap<u32,PingsValues>>);

#[derive(Resource, Default)]
pub struct PingPorts(pub(crate) HashMap<u32, HashMap<u32, PingPortsData>>);

#[allow(dead_code)]
#[derive(SystemParam)]
pub struct Ping<'w>{
    pings: NetRes<'w, Pings>,
    time: NetRes<'w, Time>,
}

#[allow(dead_code)]
impl<'w> Ping<'w> {
    fn get_ping(&self, connection_id: u32, port_id: u32) -> f32 {
        if let Some(pings_list) = self.pings.0.get(&connection_id)
        && let Some(pings_values) = pings_list.get(&port_id)
        {
            return pings_values.get_ping_without_frame_delay(&self.time);
        }

        0.0
    }

    fn get_real_ping(&self, connection_id: u32, port_id: u32) -> f32 {
        if let Some(pings_list) = self.pings.0.get(&connection_id)
            && let Some(pings_values) = pings_list.get(&port_id)
        {
            return pings_values.get_real_ping();
        }

        0.0
    }
}

impl PingsValues{
    fn get_real_ping(&self) -> f32 {
        self.ping_ms
    }

    fn get_ping_without_frame_delay(&self, time: &Time) -> f32 {
        let frame_time_ms = time.delta_secs() * 1000.0;

        (self.ping_ms - frame_time_ms).max(0.0)
    }
}

impl PingPorts {
    pub fn add_ping_port(&mut self, connection_id: u32, port_id: u32, port_args: Option<SendArgs>) {
        if let Some(current_connections_list) = self.0.get_mut(&connection_id) {
            current_connections_list.insert(port_id, PingPortsData {
                port_args
            });
        }else {
            self.0.insert(connection_id, HashMap::from([(port_id, PingPortsData { port_args })]));
        }
    }

    pub fn remove_ping_port(&mut self, connection_id: u32, port_id: u32){
        if let Some(current_connections_list) = self.0.get_mut(&connection_id) {
            current_connections_list.remove(&port_id);

            if current_connections_list.is_empty() {
                self.0.remove(&connection_id);
            }
        }
    }
}

impl ServerTimeData {
    pub fn get_server_time_now(&self, time: &Time) -> f64 {
        time.elapsed_secs_f64() + self.offset_secs
    }

    pub fn update_offset(&mut self, calculated_offset: f64) {
        if !self.is_synced {
            self.offset_secs = calculated_offset;
            self.is_synced = true;
        } else {
            let alpha = 0.1;
            self.offset_secs = (alpha * calculated_offset) + ((1.0 - alpha) * self.offset_secs);
        }
    }
}

impl ServerTime {
    pub fn get_server_time_now(&self, connection_id: u32, port_id: u32, time: &Time) -> f64 {
        if let Some(ports) = self.0.get(&connection_id)
        && let Some(server_time_data) = ports.get(&port_id)
        {
            return  server_time_data.get_server_time_now(time)
        }

        0.0
    }
}

impl Plugin for ClientPing {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pings>();
        app.init_resource::<PingPorts>();
        app.init_resource::<ServerTime>();
        app.register_message::<PingMessage>();
        app.register_message::<PongMessage>();
        app.add_systems(First,handle_client_ping.after(check_messages_from_server));
        app.add_systems(PreUpdate,(connection_closed,port_disconnected).chain());
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_server_time_offset(
    ping: &PingMessage,
    server_time: &mut ServerTimeData,
    client_now: f64,
    ping_ports: &PingPorts,
    connection_id: u32,
    port_id: u32,
    client_params: &mut ClientConnectionParams,
    local_session_uuid: &LocalSessionUUID,
    pings: &mut Pings
) {
    let one_way_delay_secs = (ping.rtt_ms / 2.0 / 1000.0) as f64;
    let estimated_server_now = ping.server_timestamp + one_way_delay_secs;
    let calculated_offset = estimated_server_now - client_now;
    let send_args = if let Some(ping_list) = ping_ports.0.get(&connection_id)
        && let Some(ping_port_data) = ping_list.get(&port_id) { ping_port_data.port_args.as_ref() } else { None };

    server_time.update_offset(calculated_offset);

    client_params.send_message(connection_id,port_id,PongMessage {
        sequence_id: ping.sequence_id,
        server_timestamp: ping.server_timestamp,
    },local_session_uuid.0,send_args);

    if let Some(ping_list) = pings.0.get_mut(&connection_id) {
        if let Some(pings_values) = ping_list.get_mut(&port_id) {
            pings_values.ping_ms = ping.rtt_ms;
        }else {
            ping_list.insert(port_id,PingsValues{
                ping_ms: ping.rtt_ms,
            });
        }
    }else {
        pings.0.insert(connection_id,HashMap::from([(port_id, PingsValues{ping_ms: ping.rtt_ms})]));
    }
}

pub fn handle_client_ping(
    mut ping_reader: MessageReader<MessageReceivedFromServer<PingMessage>>,
    time: NetRes<Time>,
    mut server_time: NetResMut<ServerTime>,
    mut client_params: ClientConnectionParams,
    mut pings: NetResMut<Pings>,
    local_session_uuid: NetRes<LocalSessionUUID>,
    ping_ports: NetRes<PingPorts>,
) {
    let client_now = time.elapsed_secs_f64();

    for ev in ping_reader.read() {
        if let Some(server_time_connection) = server_time.0.get_mut(&ev.connection_id)
        {
            if let Some(server_time) = server_time_connection.get_mut(&ev.port_id) {
                apply_server_time_offset(&ev.message,server_time, client_now, &ping_ports, ev.connection_id, ev.port_id, &mut client_params, &local_session_uuid, &mut pings);
            }else {
                let mut server_time_data = ServerTimeData{
                    offset_secs: 0.0,
                    is_synced: false,
                };

                apply_server_time_offset(&ev.message,&mut server_time_data, client_now, &ping_ports, ev.connection_id, ev.port_id, &mut client_params, &local_session_uuid, &mut pings);

                server_time_connection.insert(ev.port_id, server_time_data);
            }
        }else {
            let mut server_time_data = ServerTimeData{
                offset_secs: 0.0,
                is_synced: false,
            };

            apply_server_time_offset(&ev.message,&mut server_time_data, client_now, &ping_ports, ev.connection_id, ev.port_id, &mut client_params, &local_session_uuid, &mut pings);

            server_time.0.insert(ev.connection_id,HashMap::from([
                (ev.port_id,server_time_data)
            ]));
        }
    }
}

pub fn port_disconnected(
    mut client_port_disconnected: MessageReader<ClientPortDisconnected>,
    mut pings: NetResMut<Pings>,
    mut ping_ports: NetResMut<PingPorts>,
    mut server_time: NetResMut<ServerTime>
){
    for ev in client_port_disconnected.read() {
        if let Some(ping_list) = pings.0.get_mut(&ev.connection_id) {
            ping_list.remove(&ev.port_id);

            if ping_list.is_empty() {
                pings.0.remove(&ev.connection_id);
            }
        }

        if let Some(ping_ports_list) = ping_ports.0.get_mut(&ev.port_id) {
            ping_ports_list.remove(&ev.port_id);

            if ping_ports_list.is_empty() {
                ping_ports.0.remove(&ev.connection_id);
            }
        }

        if let Some(server_time_ports) = server_time.0.get_mut(&ev.connection_id) {
            server_time_ports.remove(&ev.port_id);

            if server_time_ports.is_empty() {
                server_time.0.remove(&ev.connection_id);
            }
        }
    }
}

pub fn connection_closed(
    mut connections_closed: MessageReader<ConnectionClosedClient>,
    mut pings: NetResMut<Pings>,
    mut ping_ports: NetResMut<PingPorts>,
    mut server_time: NetResMut<ServerTime>
){
    for ev in connections_closed.read() {
        pings.0.remove(&ev.connection_id);
        ping_ports.0.remove(&ev.connection_id);
        server_time.0.remove(&ev.connection_id);
    }
}