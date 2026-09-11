use bevy::app::{App};
use bevy::DefaultPlugins;

#[cfg(not(target_arch = "wasm32"))]
pub mod not_wasm_uses {
    pub(crate) use networkinator::shared::plugins::messaging::{MessageReceivedFromPeer, MessageTrait, MessageTraitPlugin};
    pub(crate) use bevy::prelude::{MessageReader, Startup, Update};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use message_pro_macro::ConnectionMessage;
    pub(crate) use networkinator::client::plugins::network::ClientNetworkPlugin;
    pub(crate) use networkinator::client::ports::tcp::TcpClientSettings;
    pub(crate) use networkinator::{NetRes, NetResMut};
    pub(crate) use networkinator::server::plugins::network::ServerNetworkPlugin;
    pub(crate) use networkinator::server::ports::tcp::TcpServerSettings;
    pub(crate) use networkinator::shared::plugins::authentication::{AuthenticationPlugin, ClientPortAuthenticated};
    pub(crate) use networkinator::shared::plugins::messaging::{ClientConnectionParams, MessagingPlugin};
    pub(crate) use networkinator::shared::plugins::network::{ClientConnection, DefaultNetworkPortSharedInfosClient, DefaultNetworkPortSharedInfosServer, LocalSessionUUID, NetworkConnection, NetworkPlugin, ServerConnection};
}

#[cfg(target_arch = "wasm32")]
pub mod wasm_uses {
    pub(crate) use bevy::log::warn;
    pub(crate) use networkinator::shared::plugins::messaging::{MessageTrait, MessageTraitPlugin};
    pub(crate) use bevy::prelude::{MessageReader, Startup, Update};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use message_pro_macro::ConnectionMessage;
    pub(crate) use networkinator::client::plugins::network::ClientNetworkPlugin;
    pub(crate) use networkinator::client::ports::wasm_websocket::WasmWebSocketClientSettings;
    pub(crate) use networkinator::{NetRes, NetResMut};
    pub(crate) use networkinator::shared::plugins::authentication::{AuthenticationPlugin, ClientPortAuthenticated};
    pub(crate) use networkinator::shared::plugins::messaging::{ClientConnectionParams, MessagingPlugin};
    pub(crate) use networkinator::shared::plugins::network::{ClientConnection, DefaultNetworkPortSharedInfosClient, LocalSessionUUID, NetworkConnection, NetworkPlugin};
}

#[cfg(not(target_arch = "wasm32"))]
use not_wasm_uses::*;

#[cfg(target_arch = "wasm32")]
use wasm_uses::*;

#[derive(Serialize,Deserialize,ConnectionMessage)]
pub struct HiMessage(String);

#[cfg(not(target_arch = "wasm32"))]
fn start_connection(
    mut client_network_connection: NetResMut<NetworkConnection<ClientConnection>>,
    mut server_network_connection: NetResMut<NetworkConnection<ServerConnection>>,
) {
    client_network_connection.start_connection::<DefaultNetworkPortSharedInfosClient>(0, Box::new(TcpClientSettings::default()),true);
    server_network_connection.start_connection::<DefaultNetworkPortSharedInfosServer>(0, 0, Box::new(TcpServerSettings::default()),true);
}

#[cfg(target_arch = "wasm32")]
fn start_connection(
    mut client_network_connection: NetResMut<NetworkConnection<ClientConnection>>,
) {
    client_network_connection.start_connection::<DefaultNetworkPortSharedInfosClient>(0, Box::new(WasmWebSocketClientSettings::default()),true);
}

fn send_hi_message(
    mut client_port_authenticated: MessageReader<ClientPortAuthenticated>,
    mut client_connection_params: ClientConnectionParams,
    local_session_uuid: NetRes<LocalSessionUUID>,
){
    for event in client_port_authenticated.read() {
        client_connection_params.send_message::<HiMessage>(event.connection_id, event.port_id, HiMessage("Hi server".parse().unwrap()), local_session_uuid.get_session_uuid(), None);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_hi_message(
    mut client_port_connected: MessageReader<MessageReceivedFromPeer<HiMessage>>,
){
    for event in client_port_connected.read() {
        println!("Message from client: {:?}, on port {}, from connection {}", event.message.0, event.port_id, event.connection_id);
    }
}


fn main() {
    let mut app = App::new();

    #[cfg(not(target_arch = "wasm32"))] {
        app.add_plugins((DefaultPlugins,ClientNetworkPlugin,ServerNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin));
        app.add_systems(Startup,start_connection);
        app.add_systems(Update,(send_hi_message,read_hi_message));
        app.register_message::<HiMessage>();
    }

    #[cfg(target_arch = "wasm32")] {
        warn!("Server doesn't work on WASM");
        app.add_plugins((DefaultPlugins,ClientNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin));
        app.add_systems(Startup, start_connection);
        app.add_systems(Update,send_hi_message);
        app.register_message::<HiMessage>();
    }

    app.run();
}
