pub mod client;
pub mod host;
pub mod replicate;

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{NetcodeClientTransport, NetcodeServerTransport},
    RenetClient, RenetServer, RenetServerEvent, RepliconRenetPlugins,
};

pub const PROTOCOL_ID: u64 = 7_777_001;
pub const DEFAULT_PORT: u16 = 7777;

#[derive(Resource, Clone)]
pub enum NetworkRole {
    None,
    Host { port: u16, local_ip: String },
    Client { address: String, error: Option<String> },
}

impl Default for NetworkRole {
    fn default() -> Self {
        Self::None
    }
}

impl NetworkRole {
    pub fn set_singleplayer(&mut self) {
        *self = Self::None;
    }

    pub fn set_host(&mut self, port: u16) {
        *self = Self::Host {
            port,
            local_ip: local_ip_address(),
        };
    }

    pub fn set_client(&mut self, address: String) {
        *self = Self::Client {
            address,
            error: None,
        };
    }

    pub fn is_client(&self) -> bool {
        matches!(self, Self::Client { .. })
    }

    pub fn is_host(&self) -> bool {
        matches!(self, Self::Host { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "Singleplayer",
            Self::Host { .. } => "Host",
            Self::Client { .. } => "Client",
        }
    }

    pub fn display_address(&self) -> Option<String> {
        match self {
            Self::Host { port, local_ip } => Some(format!("{local_ip}:{port}")),
            Self::Client { address, .. } => Some(address.clone()),
            Self::None => None,
        }
    }

    pub fn last_error(&self) -> Option<String> {
        match self {
            Self::Client { error, .. } => error.clone(),
            _ => None,
        }
    }

    pub fn set_error(&mut self, message: String) {
        if let Self::Client { error, .. } = self {
            *error = Some(message);
        }
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RepliconPlugins)
            .add_plugins(RepliconRenetPlugins)
            .add_plugins(replicate::ReplicatePlugin)
            .add_systems(OnEnter(crate::AppState::InGame), start_networking)
            .add_systems(OnExit(crate::AppState::InGame), stop_networking)
            .add_observer(monitor_disconnects)
            .add_systems(Update, client::handle_client_connection_errors);
    }
}

fn start_networking(
    mut commands: Commands,
    mut role: ResMut<NetworkRole>,
    channels: Res<RepliconChannels>,
) {
    let result = match role.clone() {
        NetworkRole::Host { port, .. } => host::start_host(&mut commands, &channels, port),
        NetworkRole::Client { address, .. } => client::start_client(&mut commands, &channels, &address),
        NetworkRole::None => Ok(()),
    };
    if let Err(err) = result {
        warn!("network startup failed: {err}");
        role.set_error(err.to_string());
    }
}

fn stop_networking(mut commands: Commands) {
    commands.remove_resource::<RenetServer>();
    commands.remove_resource::<NetcodeServerTransport>();
    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();
}

fn monitor_disconnects(
    event: On<RenetServerEvent>,
    role: Res<NetworkRole>,
) {
    if let bevy_replicon_renet::renet::ServerEvent::ClientDisconnected { .. } = event.0 {
        if role.is_host() {
            info!("client disconnected");
        }
    }
}

pub fn local_ip_address() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    Ipv4Addr::LOCALHOST.to_string()
}

pub fn parse_address(address: &str) -> Result<SocketAddr, String> {
    if let Ok(addr) = address.parse::<SocketAddr>() {
        return Ok(addr);
    }
    let mut parts = address.split(':');
    let ip = parts
        .next()
        .ok_or_else(|| "missing IP address".to_string())?;
    let port = parts
        .next()
        .unwrap_or(&DEFAULT_PORT.to_string())
        .parse::<u16>()
        .map_err(|_| "invalid port".to_string())?;
    let ip: IpAddr = ip.parse().map_err(|_| "invalid IP address".to_string())?;
    Ok(SocketAddr::new(ip, port))
}

pub fn current_time() -> Result<std::time::Duration> {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|err| err.into())
}
