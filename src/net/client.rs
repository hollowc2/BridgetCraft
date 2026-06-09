use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{ClientAuthentication, NetcodeClientTransport},
    renet::ConnectionConfig,
    RenetChannelsExt, RenetClient,
};
use std::net::{Ipv4Addr, UdpSocket};

use crate::net::{current_time, parse_address, NetworkRole, PROTOCOL_ID};

pub fn start_client(
    commands: &mut Commands,
    channels: &RepliconChannels,
    address: &str,
) -> Result<()> {
    let server_addr = parse_address(address).map_err(|err| {
        format!("could not parse join address '{address}': {err}")
    })?;

    let client = RenetClient::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    });

    let current_time = current_time()?;
    let client_id = current_time.as_millis() as u64;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id: PROTOCOL_ID,
        server_addr,
        user_data: None,
    };
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)?;

    commands.insert_resource(client);
    commands.insert_resource(transport);
    info!("connecting to {server_addr}");
    Ok(())
}

pub fn handle_client_connection_errors(
    mut role: ResMut<NetworkRole>,
    client: Option<Res<RenetClient>>,
) {
    if !role.is_client() {
        return;
    }
    if client.is_none() {
        role.set_error("Can't find host".to_string());
    }
}
