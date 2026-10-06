use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::io::{Error};
use std::marker::PhantomData;
use bevy::app::{App, Plugin};
use bevy::asset::uuid::Uuid;
use bevy::ecs::system::SystemParam;
use bevy::prelude::{Commands, Event, First, IntoScheduleConfigs, Message, MessageWriter, Messages, On, Resource};
use bevy::tasks::ConditionalSend;
use erased_serde::{serialize_trait_object, Serialize as ErasedSerialize};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use crate::{NetRes, NetResMut, PeersDroppedType};
use crate::client::plugins::network::check_port_disconnected as client_port_disconnected;
use crate::server::plugins::network::check_port_disconnected as server_port_disconnected;
use crate::shared::plugins::network::{BytesReceivedFromPeer, BytesReceivedFromServer, ClientConnection, ConnectionClosedClient, ConnectionClosedServer, CurrentNetworkSides, LocalPeerUUID, LocalSessionUUID, NetworkConnection, NetworkType, PeersManuallyDropped, PortClosedManuallyClient, PortClosedManuallyServer, ServerConnection};

#[cfg(target_arch = "wasm32")]
pub type SendArgs = Box<dyn Any + Send>;

#[cfg(not(target_arch = "wasm32"))]
pub type SendArgs = Box<dyn Any + Send + Sync>;

#[cfg(target_arch = "wasm32")]
pub trait MessageTrait: 'static + ErasedSerialize + ConditionalSend + Send + Sync {
    fn deserialize_message(data: &[u8]) -> Option<Self> where Self: Sized + DeserializeOwned;
    fn as_authentication(&self) -> bool {
        false
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub trait MessageTrait: 'static + ErasedSerialize + ConditionalSend + Send + Sync {
    fn deserialize_message(data: &[u8]) -> Option<Self> where Self: Sized;
    fn serialize_message(&self) -> Vec<u8>;
    fn as_authentication(&self) -> bool {
        false
    }
}

serialize_trait_object!(MessageTrait);

pub struct MessagingPlugin;

pub struct MessageFunctionsServer{
    dispatch_message: fn(commands: &mut Commands, message_bytes: Vec<u8>, connection_id: u32, port_id: u32, peer_uuid: Option<Uuid>, session_id: Uuid),
}

pub struct MessageFunctionsClient{
    dispatch_message: fn(commands: &mut Commands, message_bytes: Vec<u8>, connection_id: u32, port_id: u32)
}

pub trait MessageTraitPlugin{
    fn register_message<T: MessageTrait + DeserializeOwned>(&mut self);
}

#[derive(Serialize,Deserialize)]
pub struct MessageInfos {
    pub message_id: u32,
    pub message: Vec<u8>,
}

#[derive(Resource, Default)]
pub struct MessagesRegistryClient(u32, HashMap<u32, MessageFunctionsClient>, HashMap<TypeId, u32>);

#[derive(Resource, Default)]
pub struct MessagesRegistryServer(u32, HashMap<u32, MessageFunctionsServer>, HashMap<TypeId, u32>);

#[derive(SystemParam)]
pub struct ServerConnectionParams<'w, 's> {
    messages_registry: NetRes<'w, MessagesRegistryServer>,
    connection: NetResMut<'w, NetworkConnection<ServerConnection>>,
    local_peer_uuid: Option<NetRes<'w, LocalPeerUUID>>,
    local_session_uuid: Option<NetRes<'w, LocalSessionUUID>>,
    commands: Commands<'w, 's>,
    connection_closed: MessageWriter<'w, ConnectionClosedServer>,
    port_closed_manually: MessageWriter<'w, PortClosedManuallyServer>,
    peers_manually_dropped: MessageWriter<'w, PeersManuallyDropped>
}

#[derive(SystemParam)]
pub struct ClientConnectionParams<'w, 's> {
    messages_registry: NetRes<'w, MessagesRegistryClient>,
    connection: NetResMut<'w, NetworkConnection<ClientConnection>>,
    local_peer_uuid: NetRes<'w, LocalPeerUUID>,
    commands: Commands<'w, 's>,
    connection_closed: MessageWriter<'w, ConnectionClosedClient>,
    port_closed_manually: MessageWriter<'w, PortClosedManuallyClient>
}

#[derive(Message)]
pub struct MessageReceivedFromPeer<T: MessageTrait>{
    pub message: T,
    pub peer_uuid: Uuid,
    pub session_uuid: Uuid,
    pub port_id: u32,
    pub connection_id: u32
}

#[derive(Message)]
pub struct MessageReceivedFromAnonymousPeer<T: MessageTrait>{
    pub message: T,
    pub session_uuid: Uuid,
    pub port_id: u32,
    pub connection_id: u32
}

#[derive(Message)]
pub struct MessageReceivedFromServer<T: MessageTrait>{
    pub message: T,
    pub port_id: u32,
    pub connection_id: u32
}

#[derive(Event)]
pub struct MessageTriggerFromServer<T: MessageTrait> {
    pub message_bytes: Vec<u8>,
    pub port_id: u32,
    pub connection_id: u32,
    phantom: PhantomData<T>
}

#[derive(Event)]
pub struct MessageTriggerFromPeer<T: MessageTrait> {
    pub message_bytes: Vec<u8>,
    pub port_id: u32,
    pub connection_id: u32,
    pub peer_uuid: Option<Uuid>,
    pub session_uuid: Uuid,
    phantom: PhantomData<T>
}

#[allow(unused)]
impl<'w, 's> ServerConnectionParams<'w, 's> {
    pub fn send_message<T: MessageTrait>(&mut self, connection_id: u32, port_id: u32, message: T, peer_id: Uuid, send_args: Option<&SendArgs>){
        let type_id = TypeId::of::<T>();

        if let Some(message_id) = self.messages_registry.2.get(&type_id) {
            if let Some(local_peer_uuid) = &self.local_peer_uuid
                && let Some(local_peer_uuid) = &local_peer_uuid.0
                && local_peer_uuid == &peer_id
            {
                self.commands.trigger(MessageTriggerFromServer::<T>{
                    message_bytes: message.serialize_message(),
                    port_id,
                    connection_id,
                    phantom: Default::default(),
                });

                return;
            }

            self.connection.send_message(*message_id, connection_id, port_id, &message, peer_id, send_args);
        }
    }

    pub fn send_message_non_authenticated<T: MessageTrait>(&mut self, connection_id: u32, port_id: u32, message: T, session_uuid: Uuid, send_args: Option<&SendArgs>){
        let type_id = TypeId::of::<T>();

        if let Some(message_id) = self.messages_registry.2.get(&type_id) {
            if let Some(local_session_uuid) = &self.local_session_uuid
                && let Some(local_session_uuid) = &local_session_uuid.0
                && local_session_uuid == &session_uuid
            {
                self.commands.trigger(MessageTriggerFromServer::<T>{
                    message_bytes: message.serialize_message(),
                    port_id,
                    connection_id,
                    phantom: Default::default(),
                });

                return;
            }

            self.connection.send_message(*message_id, connection_id, port_id, &message, session_uuid, send_args);
        }
    }

    pub fn send_message_for_list<T: MessageTrait>(&mut self, connection_id: u32, port_id: u32, message: T, send_args: Option<&SendArgs>, list: Vec<Uuid>){
        let type_id = TypeId::of::<T>();

        if let Some(message_id) = self.messages_registry.2.get(&type_id) {
            let local_peer_uuid = if let Some(local_peer_uuid) = &self.local_peer_uuid { &local_peer_uuid.0 } else { &None };
            let local_session_uuid = if let Some(local_session_uuid) = &self.local_session_uuid { &local_session_uuid.0 } else { &None };
            let mut found_local_uuid = false;

            for uuid in list{
                if let Some(local_peer_uuid) = local_peer_uuid
                    && &uuid == local_peer_uuid
                {
                    found_local_uuid = true;
                    continue;
                }

                if let Some(local_session_uuid) = local_session_uuid
                    && &uuid == local_session_uuid
                {
                    found_local_uuid = true;
                    continue;
                }

                self.connection.send_message(*message_id, connection_id, port_id, &message, uuid, send_args);
            }

            if found_local_uuid {
                self.commands.trigger(MessageTriggerFromServer::<T>{
                    message_bytes: message.serialize_message(),
                    port_id,
                    connection_id,
                    phantom: Default::default(),
                });
            }
        }
    }

    pub fn send_message_for_all<T: MessageTrait>(&mut self, connection_id: u32, port_id: u32, message: T, just_authenticated: bool, send_args: Option<&SendArgs>, exceptions: Vec<Uuid>){
        let type_id = TypeId::of::<T>();

        if let Some(message_id) = self.messages_registry.2.get(&type_id) {
            let local_peer_uuid = if let Some(local_peer_uuid) = &self.local_peer_uuid { &local_peer_uuid.0 } else { &None };
            let local_session_uuid = if let Some(local_session_uuid) = &self.local_session_uuid { &local_session_uuid.0 } else { &None };

            self.connection.send_message_to_all_peer(*message_id, connection_id, port_id, &message, local_peer_uuid, just_authenticated, send_args, &exceptions);

            if let Some(local_peer_uuid) = local_peer_uuid
                && !exceptions.contains(local_peer_uuid)
            {
                self.commands.trigger(MessageTriggerFromServer::<T>{
                    message_bytes: message.serialize_message(),
                    port_id,
                    connection_id,
                    phantom: Default::default(),
                });

                return;
            }

            if let Some(local_session_uuid) = local_session_uuid
                && !exceptions.contains(local_session_uuid)
            {
                self.commands.trigger(MessageTriggerFromServer::<T>{
                    message_bytes: message.serialize_message(),
                    port_id,
                    connection_id,
                    phantom: Default::default(),
                });
            }
        }
    }

    pub fn get_connections(&mut self) -> &mut NetResMut<'w, NetworkConnection<ServerConnection>> {
        &mut self.connection
    }

    pub fn close_connection(&mut self, connection_id: u32) {
        if let Some(server_connection) = self.connection.0.get(&connection_id) {
            let mut was_connected_list: HashMap<u32,bool> = HashMap::new();
            let mut peers_list: HashMap<u32,PeersDroppedType>  = HashMap::new();
            let secondary_ports = server_connection.get_immutable_secondary_ports();

            if let Some(main_port) = server_connection.get_immutable_port(0) {
                was_connected_list.insert(0,main_port.get_port_status().first_started);

                let peers_sessions = main_port.get_all_sessions();
                let mut hash_map_insert: PeersDroppedType = HashMap::new();

                for (uuid,peer_uuid) in peers_sessions {
                    hash_map_insert.insert(uuid,(peer_uuid,Error::other("Server disconnected"),true));
                }
                
                peers_list.insert(0,hash_map_insert);
            }

            for (port_id, port) in secondary_ports.iter() {
                was_connected_list.insert(*port_id,port.get_port_status().first_started);

                let peers_sessions = port.get_all_sessions();
                let mut hash_map_insert: HashMap<Uuid,(Option<Uuid>,Error,bool)> = HashMap::new();

                for (uuid,peer_uuid) in peers_sessions {
                    hash_map_insert.insert(uuid,(peer_uuid,Error::other("Server disconnected"),true));
                }
                
                peers_list.insert(*port_id,hash_map_insert);
            }

            self.connection.close_connection(connection_id);

            self.connection_closed.write(ConnectionClosedServer {
                connection_id
            });

            for (port_id, was_started) in was_connected_list.iter() {
                self.port_closed_manually.write(PortClosedManuallyServer{
                    connection_id,
                    port_id: *port_id,
                    was_started: *was_started
                });

                if let Some(peers) = peers_list.remove(port_id) {
                    self.peers_manually_dropped.write(PeersManuallyDropped{
                        peers,
                        connection_id,
                        port_id: *port_id
                    });
                }
            }
        }
    }
    
    pub fn close_port(&mut self, connection_id: u32, port_id: u32){
        if port_id == 0 {
            self.close_connection(connection_id);
        }else{
            let mut was_started = false;
            let mut peers: HashMap<Uuid,(Option<Uuid>,Error,bool)> = HashMap::new();

            if let Some(server_connection) = self.connection.0.get(&connection_id)
            && let Some(port) = server_connection.get_immutable_port(port_id)
            {
                was_started = port.get_port_status().first_started;

                let peers_sessions = port.get_all_sessions();
                
                for (uuid,peer_uuid) in peers_sessions {
                    peers.insert(uuid,(peer_uuid,Error::other("Server disconnected"),true));
                }
            }

            self.connection.close_port(connection_id, port_id);

            self.port_closed_manually.write(PortClosedManuallyServer{
                connection_id,
                port_id,
                was_started
            });

           self.peers_manually_dropped.write(PeersManuallyDropped{
               peers,
               connection_id,
               port_id,
           });
        }
    }
    
    fn drop_peer_from_all_ports(&mut self, connection_id: u32, uuid: Uuid){
        let disconnected = self.connection.disconnect_peer_or_session(connection_id, &uuid);

        for (port_id,(season_uuid,peer_id)) in disconnected {
            self.peers_manually_dropped.write(PeersManuallyDropped{
                peers: HashMap::from([
                    (season_uuid,(peer_id,Error::other("Server disconnected"),true)),
                ]),
                connection_id,
                port_id,
            });
        }
    }
    
    fn drop_peer_from_port(&mut self, connection_id: u32, port_id: u32, uuid: Uuid){
        if let Some(server_connection) = self.connection.0.get_mut(&connection_id) 
        && let Some(port) = server_connection.get_port(port_id)
        {
            let disconnected = port.disconnect_peer_or_session(&uuid);
            
            if let Some(disconnected) = disconnected {
                self.peers_manually_dropped.write(PeersManuallyDropped{
                    peers: HashMap::from([
                        (disconnected.0,(disconnected.1,Error::other("Server disconnected"),true)),
                    ]),
                    connection_id,
                    port_id,
                });
            }
        }
    }
}

#[allow(unused)]
impl<'w, 's> ClientConnectionParams<'w, 's> {
    pub fn send_message<T: MessageTrait>(&mut self, connection_id: u32, port_id: u32, message: T, local_session_uuid: Option<Uuid>, send_args: Option<&SendArgs>){
        let type_id = TypeId::of::<T>();

        if let Some(message_id) = self.messages_registry.2.get(&type_id)
            && self.connection.send_message_to_server(*message_id, connection_id, port_id, &message, local_session_uuid, send_args)
            && let Some(local_session_uuid) = local_session_uuid {

            self.commands.trigger(MessageTriggerFromPeer::<T>{
                message_bytes: message.serialize_message(),
                port_id,
                connection_id,
                peer_uuid: self.local_peer_uuid.0,
                session_uuid: local_session_uuid,
                phantom: Default::default(),
            });
        }
    }

    pub fn get_connections(&mut self) -> &mut NetResMut<'w, NetworkConnection<ClientConnection>> {
        &mut self.connection
    }

    pub fn close_connection(&mut self, connection_id: u32) {
        if let Some(client_connection) = self.connection.0.get(&connection_id) {
            let mut was_connected_list: HashMap<u32,bool> = HashMap::new();
            let secondary_ports = client_connection.get_immutable_secondary_ports();

            if let Some(main_port) = client_connection.get_immutable_port(0) {
                was_connected_list.insert(0,main_port.get_port_status().first_started);
            }

            for (port_id, port) in secondary_ports.iter() {
                was_connected_list.insert(*port_id,port.get_port_status().first_started);
            }

            self.connection.close_connection(connection_id);

            self.connection_closed.write(ConnectionClosedClient {
                connection_id
            });

            for (port_id, was_started) in was_connected_list.iter() {
                self.port_closed_manually.write(PortClosedManuallyClient{
                    connection_id,
                    port_id: *port_id,
                    was_started: *was_started
                });
            }
        }
    }

    pub fn close_port(&mut self, connection_id: u32, port_id: u32){
        if port_id == 0 {
            self.close_connection(connection_id);
        }else{
            let mut was_started = false;

            if let Some(client_connection) = self.connection.0.get(&connection_id)
                && let Some(port) = client_connection.get_immutable_port(port_id)
            {
                was_started = port.get_port_status().first_started;
            }

            self.connection.close_port(connection_id, port_id);

            self.port_closed_manually.write(PortClosedManuallyClient{
                connection_id,
                port_id,
                was_started
            });
        }
    }
}

impl Plugin for MessagingPlugin {
    fn build(&self, app: &mut App) {
        let (is_client, is_local_server, is_dedicated_server) = {
            let world = app.world();
            let sides = world.get_resource::<CurrentNetworkSides>()
                .expect("Insert ServerNetworkPlugin or ClientNetworkPlugin first, if its a LocalServer insert both first");
            (
                sides.0.contains(&NetworkType::Client),
                sides.0.contains(&NetworkType::LocalServer),
                sides.0.contains(&NetworkType::DedicatedServer)
            )
        };

        if is_client || is_local_server {
            app.init_resource::<MessagesRegistryClient>();

            app.add_systems(First,check_messages_from_server.after(client_port_disconnected));

            if is_local_server {
                app.init_resource::<MessagesRegistryServer>();

                app.add_systems(First,check_messages_from_client.after(server_port_disconnected));
            }
        }else if is_dedicated_server {
            app.init_resource::<MessagesRegistryServer>();
            app.add_systems(First,check_messages_from_client.after(server_port_disconnected));
        }
    }
}

impl MessageTraitPlugin for App {
    fn register_message<T: MessageTrait + DeserializeOwned>(&mut self) {
        let (is_client, is_local_server, is_dedicated_server) = {
            let world = self.world();
            let sides = world.get_resource::<CurrentNetworkSides>()
                .expect("Insert ServerNetworkPlugin or ClientNetworkPlugin first, if its a LocalServer insert both first");
            (
                sides.0.contains(&NetworkType::Client),
                sides.0.contains(&NetworkType::LocalServer),
                sides.0.contains(&NetworkType::DedicatedServer)
            )
        };

        let mut found_message_client = false;
        let mut found_message_server = false;

        if is_client || is_local_server {
            if self.world().get_resource::<Messages<MessageReceivedFromServer<T>>>().is_none() {
                self.add_message::<MessageReceivedFromServer<T>>();

                self.add_observer(|
                    message_trigger_from_server: On<MessageTriggerFromServer<T>>,
                    mut message_received_from_server: MessageWriter<MessageReceivedFromServer<T>>
                | {
                    if let Some(message) = T::deserialize_message(&message_trigger_from_server.message_bytes) {
                        message_received_from_server.write(MessageReceivedFromServer{
                            message,
                            port_id: message_trigger_from_server.port_id,
                            connection_id: message_trigger_from_server.connection_id
                        });
                    }
                });
            }else {
                found_message_client = true;
            }

            if is_local_server {
                if self.world().get_resource::<Messages<MessageReceivedFromPeer<T>>>().is_none() {
                    self.add_message::<MessageReceivedFromPeer<T>>();
                    self.add_message::<MessageReceivedFromAnonymousPeer<T>>();

                    self.add_observer(|
                        message_trigger_from_server: On<MessageTriggerFromPeer<T>>,
                        mut message_received_from_peer: MessageWriter<MessageReceivedFromPeer<T>>,
                        mut message_received_from_anonymous_peer: MessageWriter<MessageReceivedFromAnonymousPeer<T>>,
                    | {
                        if let Some(message) = T::deserialize_message(&message_trigger_from_server.message_bytes) {
                            if let Some(peer_uuid) = message_trigger_from_server.peer_uuid {
                                message_received_from_peer.write(MessageReceivedFromPeer{
                                    message,
                                    peer_uuid,
                                    session_uuid: message_trigger_from_server.session_uuid,
                                    port_id: message_trigger_from_server.port_id,
                                    connection_id: message_trigger_from_server.connection_id,
                                });
                            }else {
                                message_received_from_anonymous_peer.write(MessageReceivedFromAnonymousPeer{
                                    message,
                                    session_uuid: message_trigger_from_server.session_uuid,
                                    port_id: message_trigger_from_server.port_id,
                                    connection_id: message_trigger_from_server.connection_id,
                                });
                            }
                        }
                    });
                }else {
                    found_message_server = true;
                }
            }
        }else if is_dedicated_server {
            if self.world().get_resource::<Messages<MessageReceivedFromPeer<T>>>().is_none() {
                self.add_message::<MessageReceivedFromPeer<T>>();
                self.add_message::<MessageReceivedFromAnonymousPeer<T>>();

                self.add_observer(|
                    message_trigger_from_server: On<MessageTriggerFromPeer<T>>,
                    mut message_received_from_peer: MessageWriter<MessageReceivedFromPeer<T>>,
                    mut message_received_from_anonymous_peer: MessageWriter<MessageReceivedFromAnonymousPeer<T>>,
                | {
                    if let Some(message) = T::deserialize_message(&message_trigger_from_server.message_bytes) {
                        if let Some(peer_uuid) = message_trigger_from_server.peer_uuid {
                            message_received_from_peer.write(MessageReceivedFromPeer{
                                message,
                                peer_uuid,
                                session_uuid: message_trigger_from_server.session_uuid,
                                port_id: message_trigger_from_server.port_id,
                                connection_id: message_trigger_from_server.connection_id,
                            });
                        }else {
                            message_received_from_anonymous_peer.write(MessageReceivedFromAnonymousPeer{
                                message,
                                session_uuid: message_trigger_from_server.session_uuid,
                                port_id: message_trigger_from_server.port_id,
                                connection_id: message_trigger_from_server.connection_id,
                            });
                        }
                    }
                });
            }else {
                found_message_server = true;
            }
        }

        if is_client || is_local_server {
            if !found_message_client {
                let world = self.world_mut();

                let mut msg_registry = world
                    .get_resource_mut::<MessagesRegistryClient>()
                    .expect("MessagesRegistryClient not registered; please add MessagingPlugin first");
                let new_value = msg_registry.0 + 1;
                let type_id = TypeId::of::<T>();

                msg_registry.0 = new_value;

                msg_registry.1.insert(new_value, MessageFunctionsClient{
                    dispatch_message: dispatch_message_client::<T>
                });

                msg_registry.2.insert(type_id,new_value);
            }

            if !found_message_server {
                let world = self.world_mut();

                if is_local_server {
                    let mut msg_registry = world
                        .get_resource_mut::<MessagesRegistryServer>()
                        .expect("MessagesRegistryServer not registered; please add MessagingPlugin first");
                    let new_value = msg_registry.0 + 1;
                    let type_id = TypeId::of::<T>();

                    msg_registry.0 = new_value;

                    msg_registry.1.insert(new_value, MessageFunctionsServer{
                        dispatch_message: dispatch_message_server::<T>
                    });

                    msg_registry.2.insert(type_id,new_value);
                }
            }
        }else if is_dedicated_server && !found_message_server {
            let world = self.world_mut();

            let mut msg_registry = world
                .get_resource_mut::<MessagesRegistryServer>()
                .expect("MessagesRegistryServer not registered; please add MessagingPlugin first");
            let new_value = msg_registry.0 + 1;
            let type_id = TypeId::of::<T>();

            msg_registry.0 = new_value;

            msg_registry.1.insert(new_value, MessageFunctionsServer{
                dispatch_message: dispatch_message_server::<T>
            });

            msg_registry.2.insert(type_id,new_value);
        }
    }
}

pub fn check_messages_from_client(
    mut network_connection: NetResMut<NetworkConnection<ServerConnection>>,
    messages_registry_server: NetRes<MessagesRegistryServer>,
    mut commands: Commands,
    mut bytes_received_from_peer: MessageWriter<BytesReceivedFromPeer>
){
    for (connection_id,connection) in network_connection.0.iter_mut(){
        if let Some(main_port) = connection.get_port(0){
            for (session_uuid, (messages, peer_uuid)) in main_port.get_peers_messages() {
                for bytes in messages {
                    bytes_received_from_peer.write(BytesReceivedFromPeer{
                        peer_season_uuid: session_uuid,
                        peer_id: peer_uuid,
                        connection_id: *connection_id,
                        port_id: 0,
                        bytes_length: bytes.len(),
                    });
                    
                    main_port.pong(&session_uuid, &bytes, None);
                    
                    if let Some(message_infos) = main_port.deserialize_message_infos(bytes) && let Some(registry) = messages_registry_server.1.get(&message_infos.message_id)
                    {
                        let dispatch = registry.dispatch_message;
                        let connection_id = *connection_id;

                        dispatch(&mut commands, message_infos.message, connection_id, 0, peer_uuid, session_uuid);
                    }
                }
            }
        }

        for (port_id,port) in connection.get_secondary_ports().iter_mut() {
            for (session_uuid, (messages, peer_uuid)) in port.get_peers_messages() {
                for bytes in messages {
                    bytes_received_from_peer.write(BytesReceivedFromPeer{
                        peer_season_uuid: session_uuid,
                        peer_id: peer_uuid,
                        connection_id: *connection_id,
                        port_id: *port_id,
                        bytes_length: bytes.len(),
                    });
                    
                    port.pong(&session_uuid, &bytes, None);

                    if let Some(message_infos) = port.deserialize_message_infos(bytes) && let Some(registry) = messages_registry_server.1.get(&message_infos.message_id)
                    {
                        let dispatch = registry.dispatch_message;
                        let connection_id = *connection_id;
                        let port_id = *port_id;

                        dispatch(&mut commands, message_infos.message, connection_id, port_id, peer_uuid, session_uuid);
                    }
                }
            }
        }
    }
}

pub fn check_messages_from_server(
    mut network_connection: NetResMut<NetworkConnection<ClientConnection>>,
    messages_registry_client: NetRes<MessagesRegistryClient>,
    mut commands: Commands,
    mut bytes_received_from_server: MessageWriter<BytesReceivedFromServer>
){
    for (connection_id,connection) in network_connection.0.iter_mut(){
        if let Some(main_port) = connection.get_port(0){
            for bytes in main_port.get_server_messages() {
                bytes_received_from_server.write(BytesReceivedFromServer{
                    connection_id: *connection_id,
                    port_id: 0,
                    bytes_length: bytes.len(),
                });
                
                main_port.pong(&bytes, None);

                if let Some(message_infos) = main_port.deserialize_message_infos(bytes) && let Some(registry) = messages_registry_client.1.get(&message_infos.message_id)
                {
                    let dispatch = registry.dispatch_message;
                    let connection_id = *connection_id;

                    dispatch(&mut commands, message_infos.message, connection_id, 0);
                }
            }
        }

        for (port_id,port) in connection.get_secondary_ports().iter_mut() {
            for bytes in port.get_server_messages() {
                bytes_received_from_server.write(BytesReceivedFromServer{
                    connection_id: *connection_id,
                    port_id: *port_id,
                    bytes_length: bytes.len(),
                });
                
                port.pong(&bytes, None);

                if let Some(message_infos) = port.deserialize_message_infos(bytes) && let Some(registry) = messages_registry_client.1.get(&message_infos.message_id)
                {
                    let dispatch = registry.dispatch_message;
                    let connection_id = *connection_id;
                    let port_id = *port_id;

                    dispatch(&mut commands, message_infos.message, connection_id, port_id);
                }
            }
        }
    }
}

fn dispatch_message_server<T: MessageTrait>(commands: &mut Commands, message_bytes: Vec<u8>, connection_id: u32, port_id: u32, peer_uuid: Option<Uuid>, session_uuid: Uuid)  {
    commands.trigger(MessageTriggerFromPeer::<T>{
        message_bytes,
        port_id,
        connection_id,
        peer_uuid,
        session_uuid,
        phantom: Default::default(),
    });
}

fn dispatch_message_client<T: MessageTrait>(commands: &mut Commands, message_bytes: Vec<u8>, connection_id: u32, port_id: u32)  {
    commands.trigger(MessageTriggerFromServer::<T>{
        message_bytes,
        port_id,
        connection_id,
        phantom: Default::default()
    });
}

impl MessagesRegistryServer {
    pub fn get_registers(&self) -> &HashMap<TypeId, u32> {
        &self.2
    }

    pub fn get_amount_registered(&self) -> u32 {
        self.0
    }
}

impl MessagesRegistryClient {
    pub fn get_registers(&self) -> &HashMap<TypeId, u32> {
        &self.2
    }

    pub fn get_amount_registered(&self) -> u32 {
        self.0
    }
}