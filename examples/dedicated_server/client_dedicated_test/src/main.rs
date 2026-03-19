use bevy::DefaultPlugins;
use bevy::prelude::{App, Startup};
use client::plugins::network::ClientNetworkPlugin;
use client::ports::tcp::TcpConfigsClient;
use shared::NetResMut;
use shared::plugins::authentication::AuthenticationPlugin;
use shared::plugins::messaging::MessagingPlugin;
use shared::plugins::network::{ClientConnection, NetworkConnection, NetworkPlugin};

fn start_connection(
    mut network_connection: NetResMut<NetworkConnection<ClientConnection>>,
) {
    network_connection.start_connection(0,true,Box::new(TcpConfigsClient::default()));
    network_connection.create_secondary_port(0,1,Box::new(TcpConfigsClient::default().with_port(8081)));
}

fn main() {
    App::new().add_plugins((DefaultPlugins,ClientNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin)).add_systems(Startup,start_connection).run();
}
