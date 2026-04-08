use bevy::app::Update;
use networkinator::shared::plugins::messaging::{MessageReceivedFromPeer, MessageTrait, MessageTraitPlugin};
use bevy::DefaultPlugins;
use bevy::prelude::{App, MessageReader, Startup};
use serde::{Deserialize, Serialize};
use message_pro_macro::ConnectionMessage;
use networkinator::client::plugins::network::ClientNetworkPlugin;
use networkinator::client::ports::tcp::TcpClientSettings;
use networkinator::{NetRes, NetResMut};
use networkinator::server::plugins::network::ServerNetworkPlugin;
use networkinator::server::ports::tcp::TcpServerSettings;
use networkinator::shared::plugins::authentication::{AuthenticationPlugin, ClientPortAuthenticated};
use networkinator::shared::plugins::messaging::{ClientConnectionParams, MessagingPlugin};
use networkinator::shared::plugins::network::{ClientConnection, DefaultNetworkPortSharedInfosClient, DefaultNetworkPortSharedInfosServer, LocalSessionUUID, NetworkConnection, NetworkPlugin, ServerConnection};

#[derive(Serialize,Deserialize,ConnectionMessage)]
pub struct HiMessage(String);

fn start_connection(
    mut client_network_connection: NetResMut<NetworkConnection<ClientConnection>>,
    mut server_network_connection: NetResMut<NetworkConnection<ServerConnection>>,
) {
    client_network_connection.start_connection::<DefaultNetworkPortSharedInfosClient>(0, Box::new(TcpClientSettings::default()),true);
    server_network_connection.start_connection::<DefaultNetworkPortSharedInfosServer>(0, 0, Box::new(TcpServerSettings::default()),true);
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

fn read_hi_message(
    mut client_port_connected: MessageReader<MessageReceivedFromPeer<HiMessage>>,
){
    for event in client_port_connected.read() {
        println!("Message from client: {:?}, on port {}, from connection {}", event.message.0, event.port_id, event.connection_id);
    }
}

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins,ClientNetworkPlugin,ServerNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin));
    app.add_systems(Startup,start_connection);
    app.add_systems(Update,(send_hi_message,read_hi_message));
    app.register_message::<HiMessage>();
    app.run();
}
