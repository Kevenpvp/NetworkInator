use bevy::DefaultPlugins;
use bevy::prelude::{App, Startup};
use client::plugins::network::ClientNetworkPlugin;
use client::ports::tcp::TcpClientSettings;
use server::plugins::network::ServerNetworkPlugin;
use server::ports::tcp::TcpServerSettings;
use shared::NetResMut;
use shared::plugins::authentication::AuthenticationPlugin;
use shared::plugins::messaging::MessagingPlugin;
use shared::plugins::network::{ClientConnection, DefaultNetworkPortSharedInfosClient, DefaultNetworkPortSharedInfosServer, NetworkConnection, NetworkPlugin, ServerConnection};

fn start_connection(
    mut client_network_connection: NetResMut<NetworkConnection<ClientConnection>>,
    mut server_network_connection: NetResMut<NetworkConnection<ServerConnection>>,
) {
    client_network_connection.start_connection::<DefaultNetworkPortSharedInfosClient>(0, Box::new(TcpClientSettings::default()),true);
    server_network_connection.start_connection::<DefaultNetworkPortSharedInfosServer>(0, 0, Box::new(TcpServerSettings::default()),true);
}

fn main() {
    App::new().add_plugins((DefaultPlugins,ClientNetworkPlugin,ServerNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin)).add_systems(Startup,start_connection).run();
}
