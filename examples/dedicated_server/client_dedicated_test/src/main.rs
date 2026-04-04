use bevy::DefaultPlugins;
use bevy::prelude::{App, Startup};
use client::plugins::network::ClientNetworkPlugin;
use client::ports::tcp::TcpClientSettings;
use shared::NetResMut;
use shared::plugins::authentication::AuthenticationPlugin;
use shared::plugins::messaging::MessagingPlugin;
use shared::plugins::network::{ClientConnection, DefaultNetworkPortSharedInfosClient, NetworkConnection, NetworkPlugin};

fn start_connection(
    mut network_connection: NetResMut<NetworkConnection<ClientConnection>>,
) {
    network_connection.start_connection::<DefaultNetworkPortSharedInfosClient>(0, Box::new(TcpClientSettings::default()),true);
}

fn main() {
    App::new().add_plugins((DefaultPlugins,ClientNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin)).add_systems(Startup,start_connection).run();
}
