use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig},
    renet::ConnectionConfig,
    RenetChannelsExt, RenetServer,
};
use std::net::{Ipv4Addr, UdpSocket};

use crate::net::{current_time, NetworkRole, PROTOCOL_ID};

pub fn start_host(
    commands: &mut Commands,
    channels: &RepliconChannels,
    port: u16,
) -> Result<()> {
    let server = RenetServer::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    });

    let current_time = current_time()?;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port))?;
    let server_config = ServerConfig {
        current_time,
        max_clients: 1,
        protocol_id: PROTOCOL_ID,
        authentication: ServerAuthentication::Unsecure,
        public_addresses: vec![socket.local_addr()?],
    };
    let transport = NetcodeServerTransport::new(server_config, socket)?;

    commands.insert_resource(server);
    commands.insert_resource(transport);
    info!("hosting game on port {port}");
    Ok(())
}

pub fn show_host_message(role: Res<NetworkRole>) {
    if let NetworkRole::Host { port, local_ip } = &*role {
        info!("share this address with your daughter: {local_ip}:{port}");
    }
}
