use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};
use bevy::app::App;
use bevy::asset::uuid::Uuid;
use bevy::prelude::{Commands, Message, Messages, Plugin, Resource, Update, World};
use bevy::reflect::erased_serde::__private::serde::de::DeserializeOwned;
use erased_serde::{serialize_trait_object, Serialize as ErasedSerialize};
use postcard::from_bytes;
use serde::{Deserialize, Serialize};
use crate::{NetRes, NetResMut};
use crate::plugins::network::{ClientConnection, CurrentNetworkSides, NetworkConnection, NetworkType, ServerConnection};

pub struct MessagingPlugin;

#[cfg(target_arch = "wasm32")]
pub trait MessageTrait: 'static + ErasedSerialize{}

#[cfg(not(target_arch = "wasm32"))]
pub trait MessageTrait: Send + Sync + 'static + ErasedSerialize{
    fn as_authentication(&self) -> bool {
        false
    }
}

serialize_trait_object!(MessageTrait);

#[cfg(target_arch = "wasm32")]
pub struct MessageFunctions{
    deserialize: fn(&[u8]) -> Box<dyn Any + 'static>,
    dispatch_message: fn(world: &mut World, message: Box<dyn Any>, network_type: &NetworkType, connection_id: u32, port_id: u32, peer_id: Option<Uuid>, session_id: Uuid)
}


#[cfg(not(target_arch = "wasm32"))]
pub struct MessageFunctions{
    deserialize: fn(&[u8]) -> Box<dyn Any + Send + Sync + 'static>,
    dispatch_message: fn(world: &mut World, message: Box<dyn Any>, network_type: &NetworkType, connection_id: u32, port_id: u32, peer_id: Option<Uuid>, session_id: Option<Uuid>)
}

pub(crate) static MESSAGE_REGISTRY_TYPE_ID: LazyLock<RwLock<HashMap<TypeId, u32>>> = LazyLock::new(|| {
    RwLock::new(HashMap::new())
});

#[derive(Resource, Default)]
pub struct MessagesRegistryClient(u32, HashMap<u32, MessageFunctions>);

#[derive(Resource, Default)]
pub struct MessagesRegistryServer(u32, HashMap<u32, MessageFunctions>);

#[derive(Serialize,Deserialize)]
pub struct MessageInfos {
    pub message_id: u32,
    pub message: Vec<u8>,
}

#[derive(Message)]
pub struct MessageReceivedFromServer<T: MessageTrait>{
    pub message: T,
    pub port_id: u32,
    pub connection_id: u32
}

#[derive(Message)]
pub struct MessageReceivedFromClient<T: MessageTrait>{
    pub message: T,
    pub peer_id: Option<Uuid>,
    pub session_id: Uuid,
    pub port_id: u32,
    pub connection_id: u32
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

            app.add_systems(Update,check_messages_from_server);

            if is_local_server {
                app.init_resource::<MessagesRegistryServer>();
                app.add_systems(Update,check_messages_from_client);
            }
        }else if is_dedicated_server {
            app.init_resource::<MessagesRegistryServer>();
            app.add_systems(Update,check_messages_from_client);
        }
    }
}

pub trait MessageTraitPlugin{
    fn register_message<T: MessageTrait + DeserializeOwned>(&mut self);
}

#[cfg(not(target_arch = "wasm32"))]
fn deserialize_message<T: MessageTrait + DeserializeOwned>(bytes: &[u8]) -> Box<dyn Any + Send + Sync + 'static> {
    let msg: T = from_bytes(bytes).expect("Failed to decode message");

    Box::new(msg)
}

#[cfg(target_arch = "wasm32")]
fn deserialize_message<T: MessageTrait + DeserializeOwned>(bytes: &[u8]) -> Box<dyn Any + 'static> {
    let msg: T = from_bytes(bytes).expect("Failed to decode message");

    Box::new(msg)
}

fn dispatch_message<T: MessageTrait + DeserializeOwned>(world: &mut World, message: Box<dyn Any>, network_type: &NetworkType, connection_id: u32, port_id: u32, peer_id: Option<Uuid>, session_id: Option<Uuid>)  {
    let message_downcast = message.downcast::<T>().expect("Failed to downcast");

    match network_type {
        NetworkType::Client => {
            world.write_message(MessageReceivedFromServer {
                message: *message_downcast,
                port_id,
                connection_id,
            });
        }
        _ => {
            world.write_message(MessageReceivedFromClient {
                message: *message_downcast,
                peer_id,
                session_id: session_id.unwrap(),
                port_id,
                connection_id,
            });
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
            }else {
                found_message_client = true;
            }
            
            if is_local_server {
                if self.world().get_resource::<Messages<MessageReceivedFromClient<T>>>().is_none() {
                    self.add_message::<MessageReceivedFromClient<T>>();
                }else {
                    found_message_server = true;
                }
            }
        }else if is_dedicated_server {
            if self.world().get_resource::<Messages<MessageReceivedFromClient<T>>>().is_none() {
                self.add_message::<MessageReceivedFromClient<T>>();
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

                msg_registry.1.insert(new_value, MessageFunctions{
                    deserialize: deserialize_message::<T>,
                    dispatch_message: dispatch_message::<T>,
                });

                let mut registry = MESSAGE_REGISTRY_TYPE_ID.write().unwrap();
                registry.insert(type_id, new_value);
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

                    msg_registry.1.insert(new_value, MessageFunctions{
                        deserialize: deserialize_message::<T>,
                        dispatch_message: dispatch_message::<T>,
                    });

                    let mut registry = MESSAGE_REGISTRY_TYPE_ID.write().unwrap();
                    registry.insert(type_id, new_value);
                }
            }
        }else if is_dedicated_server {
            if !found_message_server {
                let world = self.world_mut();

                let mut msg_registry = world
                    .get_resource_mut::<MessagesRegistryServer>()
                    .expect("MessagesRegistryServer not registered; please add MessagingPlugin first");
                let new_value = msg_registry.0 + 1;
                let type_id = TypeId::of::<T>();

                msg_registry.0 = new_value;

                msg_registry.1.insert(new_value, MessageFunctions{
                    deserialize: deserialize_message::<T>,
                    dispatch_message: dispatch_message::<T>,
                });

                let mut registry = MESSAGE_REGISTRY_TYPE_ID.write().unwrap();
                registry.insert(type_id, new_value);
            }
        }
    }
}

pub fn check_messages_from_client(
    mut network_connection: NetResMut<NetworkConnection<ServerConnection>>,
    messages_registry_server: NetRes<MessagesRegistryServer>,
    mut commands: Commands,
){
    for (connection_id,server_connection) in network_connection.0.iter_mut() {
        if let Some(main_port) = server_connection.get_port(0) {
            let messages = main_port.get_peers_messages();

            for (season_uuid, peer_id,bytes) in messages {
                match from_bytes::<MessageInfos>(&*bytes) {
                    Ok(message_infos) => {
                        if let Some(message_functions) = messages_registry_server.1.get(&message_infos.message_id) {
                            let message = (message_functions.deserialize)(&*message_infos.message);
                            let dispatch = message_functions.dispatch_message;
                            let connection_id = *connection_id;
                            let peer_id = peer_id;
                            let season_uuid = season_uuid;

                            commands.queue(move |world: &mut World| {
                                dispatch(world, message, &NetworkType::DedicatedServer, connection_id, 0, peer_id, Some(season_uuid));
                            })
                        }
                    }
                    Err(_) => {
                        continue;
                    }
                };
            }
        }

        for (port_id,port) in server_connection.secondary_ports.iter_mut() {
            let messages = port.get_peers_messages();
            let port_id = *port_id;

            for (season_uuid, peer_id,bytes) in messages {
                match from_bytes::<MessageInfos>(&*bytes) {
                    Ok(message_infos) => {
                        if let Some(message_functions) = messages_registry_server.1.get(&message_infos.message_id) {
                            let message = (message_functions.deserialize)(&*message_infos.message);
                            let dispatch = message_functions.dispatch_message;
                            let connection_id = *connection_id;
                            let peer_id = peer_id;
                            let season_uuid = season_uuid;

                            commands.queue(move |world: &mut World| {
                                dispatch(world, message, &NetworkType::DedicatedServer, connection_id, port_id, peer_id, Some(season_uuid));
                            })
                        }
                    }
                    Err(_) => {
                        continue;
                    }
                };
            }
        }
    }
}

pub fn check_messages_from_server(
    mut network_connection: NetResMut<NetworkConnection<ClientConnection>>,
    messages_registry_client: NetRes<MessagesRegistryClient>,
    mut commands: Commands,
){
    for (connection_id,client_connection) in network_connection.0.iter_mut() {
        if let Some(main_port) = client_connection.get_port(0) {
            let messages = main_port.get_server_messages();

            for bytes in messages {
                match from_bytes::<MessageInfos>(&*bytes) {
                    Ok(message_infos) => {
                        if let Some(message_functions) = messages_registry_client.1.get(&message_infos.message_id) {
                            let message = (message_functions.deserialize)(&*message_infos.message);
                            let dispatch = message_functions.dispatch_message;
                            let connection_id = *connection_id;

                            commands.queue(move |world: &mut World| {
                                dispatch(world, message, &NetworkType::Client, connection_id, 0, None, None);
                            })
                        }
                    }
                    Err(_) => {
                        continue;
                    }
                };
            }
        }

        for (port_id,port) in client_connection.secondary_ports.iter_mut() {
            let messages = port.get_server_messages();
            let port_id = *port_id;

            for bytes in messages {
                match from_bytes::<MessageInfos>(&*bytes) {
                    Ok(message_infos) => {
                        if let Some(message_functions) = messages_registry_client.1.get(&message_infos.message_id) {
                            let message = (message_functions.deserialize)(&*message_infos.message);
                            let dispatch = message_functions.dispatch_message;
                            let connection_id = *connection_id;

                            commands.queue(move |world: &mut World| {
                                dispatch(world, message, &NetworkType::Client, connection_id, port_id, None, None);
                            })
                        }
                    }
                    Err(_) => {
                        continue;
                    }
                };
            }
        }
    }
}