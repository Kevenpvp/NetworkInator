use shared::plugins::messaging::{MessageTrait, MessageTraitPlugin};
use bevy::DefaultPlugins;
use bevy::prelude::{App, MessageReader, Startup, Update};
use client::plugins::network::{ClientNetworkPlugin};
use client::ports::tcp::TcpClientSettings;
use serde::{Deserialize, Serialize};
use message_pro_macro::ConnectionMessage;
use shared::NetResMut;
use shared::plugins::authentication::{AuthenticationPlugin, ClientPortAuthenticated};
use shared::plugins::messaging::{ClientConnectionParams, MessagingPlugin};
use shared::plugins::network::{ClientConnection, DefaultNetworkPortSharedInfosClient, NetworkConnection, NetworkPlugin};

#[derive(Serialize,Deserialize,ConnectionMessage)]
pub struct HiMessage(String);

fn start_connection(
    mut network_connection: NetResMut<NetworkConnection<ClientConnection>>,
) {
    network_connection.start_connection::<DefaultNetworkPortSharedInfosClient>(0, Box::new(TcpClientSettings::default()),true);
    network_connection.open_secondary_port(0, Box::new(TcpClientSettings::default().with_port(8070)));
}

fn send_hi_message(
    mut client_port_authenticated: MessageReader<ClientPortAuthenticated>,
    mut client_connection_params: ClientConnectionParams
){
    for event in client_port_authenticated.read() {
        client_connection_params.send_message::<HiMessage>(event.connection_id, event.port_id, &HiMessage("Hi server".parse().unwrap()), None);
    }
}

fn main() {
    let mut app = App::new();
    
    app.add_plugins((DefaultPlugins,ClientNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin));
    app.add_systems(Startup,start_connection);
    app.add_systems(Update,send_hi_message);
    app.register_message::<HiMessage>();
    app.run();
}
