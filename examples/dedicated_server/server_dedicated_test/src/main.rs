use bevy::app::Update;
use shared::plugins::messaging::{MessageReceivedFromPeer, MessageTrait, MessageTraitPlugin};
use bevy::DefaultPlugins;
use bevy::prelude::{App, MessageReader, Startup};
use serde::{Deserialize, Serialize};
use message_pro_macro::ConnectionMessage;
use server::plugins::network::ServerNetworkPlugin;
use server::ports::tcp::TcpServerSettings;
use server::ports::udp::UdpServerSettings;
use shared::NetResMut;
use shared::plugins::authentication::AuthenticationPlugin;
use shared::plugins::messaging::MessagingPlugin;
use shared::plugins::network::{DefaultNetworkPortSharedInfosServer, NetworkConnection, NetworkPlugin, ServerConnection};

#[derive(Serialize,Deserialize,ConnectionMessage)]
pub struct HiMessage(String);

fn start_connection(
    mut network_connection: NetResMut<NetworkConnection<ServerConnection>>,
) {
    network_connection.start_connection::<DefaultNetworkPortSharedInfosServer>(0, 0, Box::new(TcpServerSettings::default()),true);
    network_connection.open_secondary_port(0, Box::new(UdpServerSettings::default().with_port(8070)));
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

    app.add_plugins((DefaultPlugins,ServerNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin));
    app.add_systems(Startup,start_connection);
    app.add_systems(Update,read_hi_message);
    app.register_message::<HiMessage>();
    app.run();
}
