use bevy::prelude::{App};
pub(crate) use bevy::DefaultPlugins;

#[cfg(target_arch = "wasm32")]
use bevy::log::warn;

#[cfg(not(target_arch = "wasm32"))]
pub mod not_wasm_uses {
    pub(crate) use networkinator::shared::plugins::messaging::{MessageReceivedFromPeer, MessageTrait, MessageTraitPlugin, MessagingPlugin};
    pub(crate) use bevy::app::Update;
    pub(crate) use bevy::prelude::{MessageReader, Startup};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use message_pro_macro::ConnectionMessage;
    pub(crate) use networkinator::NetResMut;
    pub(crate) use networkinator::server::plugins::network::ServerNetworkPlugin;
    pub(crate) use networkinator::server::ports::tcp::TcpServerSettings;
    pub(crate) use networkinator::server::ports::udp::UdpServerSettings;
    pub(crate) use networkinator::shared::plugins::authentication::AuthenticationPlugin;
    pub(crate)use networkinator::shared::plugins::network::{DefaultNetworkPortSharedInfosServer, NetworkConnection, NetworkPlugin, ServerConnection};
}

#[cfg(not(target_arch = "wasm32"))]
use not_wasm_uses::*;

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

    #[cfg(not(target_arch = "wasm32"))] {
        app.add_plugins((DefaultPlugins,ServerNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin));
        app.add_systems(Startup,start_connection);
        app.add_systems(Update,read_hi_message);
        app.register_message::<HiMessage>();
    }

    #[cfg(target_arch = "wasm32")] {
        warn!("Server doesn't work on WASM");
        app.add_plugins(DefaultPlugins);
    }

    app.run();
}
