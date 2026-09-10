#[cfg(target_arch = "wasm32")]
use bevy::prelude::{App};

#[cfg(not(target_arch = "wasm32"))] use networkinator::shared::plugins::messaging::{MessageReceivedFromPeer, MessageTrait, MessageTraitPlugin, MessagingPlugin};
#[cfg(not(target_arch = "wasm32"))] use bevy::app::Update;
#[cfg(not(target_arch = "wasm32"))] use bevy::DefaultPlugins;
#[cfg(not(target_arch = "wasm32"))] use bevy::prelude::{App, MessageReader, Startup};
#[cfg(not(target_arch = "wasm32"))] use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))] use message_pro_macro::ConnectionMessage;
#[cfg(not(target_arch = "wasm32"))] use networkinator::NetResMut;
#[cfg(not(target_arch = "wasm32"))] use networkinator::server::plugins::network::ServerNetworkPlugin;
#[cfg(not(target_arch = "wasm32"))]
use networkinator::server::ports::tcp::TcpServerSettings;
#[cfg(not(target_arch = "wasm32"))]
use networkinator::server::ports::udp::UdpServerSettings;
#[cfg(not(target_arch = "wasm32"))] use networkinator::shared::plugins::authentication::AuthenticationPlugin;
#[cfg(not(target_arch = "wasm32"))] use networkinator::shared::plugins::network::{DefaultNetworkPortSharedInfosServer, NetworkConnection, NetworkPlugin, ServerConnection};

#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize,Deserialize,ConnectionMessage)]
pub struct HiMessage(String);

#[cfg(not(target_arch = "wasm32"))]
fn start_connection(
    mut network_connection: NetResMut<NetworkConnection<ServerConnection>>,
) {
    network_connection.start_connection::<DefaultNetworkPortSharedInfosServer>(0, 0, Box::new(TcpServerSettings::default()),true);
    network_connection.open_secondary_port(0, Box::new(UdpServerSettings::default().with_port(8070)));
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

    #[cfg(not(target_arch = "wasm32"))]app.add_plugins((DefaultPlugins,ServerNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin));
    #[cfg(not(target_arch = "wasm32"))]app.add_systems(Startup,start_connection);
    #[cfg(not(target_arch = "wasm32"))]app.add_systems(Update,read_hi_message);
    #[cfg(not(target_arch = "wasm32"))]app.register_message::<HiMessage>();
    app.run();
}
