use bevy::DefaultPlugins;
use bevy::prelude::{App, Startup};
use server::plugins::network::ServerNetworkPlugin;
use server::ports::tcp::TcpServerSettings;
use shared::NetResMut;
use shared::plugins::authentication::AuthenticationPlugin;
use shared::plugins::messaging::MessagingPlugin;
use shared::plugins::network::{DefaultNetworkPortSharedInfosServer, NetworkConnection, NetworkPlugin, ServerConnection};

fn start_connection(
    mut network_connection: NetResMut<NetworkConnection<ServerConnection>>,
) {
    network_connection.start_connection::<DefaultNetworkPortSharedInfosServer>(0, 0, Box::new(TcpServerSettings::default()),true);
}

fn main() {
    App::new().add_plugins((DefaultPlugins,ServerNetworkPlugin,NetworkPlugin,MessagingPlugin,AuthenticationPlugin)).add_systems(Startup,start_connection).run();
}
