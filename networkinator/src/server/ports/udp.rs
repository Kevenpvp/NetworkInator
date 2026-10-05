#![cfg(not(target_arch = "wasm32"))]
use std::any::Any;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use bevy::asset::uuid::Uuid;
use bevy::log::warn;
use tokio::net::{UdpSocket};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use crate::shared::plugins::messaging::{MessageInfos, MessageTrait, SendArgs};
use crate::shared::plugins::network::{DefaultNetworkPortSharedInfosServer, PortReliability, PortStatus, ServerPortTrait, ServerSettingsPort};
use crate::shared::port_systems::inject_extract_uuid::extract_uuid;

pub struct UdpServerSettings {
    address: IpAddr,
    port: u16,
    buffer_size: usize,
    hook_udp_socket: Option<fn(tcp_stream: UdpSocket) -> UdpSocket>,
}

pub struct PeerConnected{
    peer_id: Option<Uuid>,
    socket_addr: SocketAddr,
    non_authenticated_instant: Instant,
    last_pong_instant: Instant,
    last_ping_instant: Instant,
    is_local: bool
}

pub struct UdpServerPort {
    settings: UdpServerSettings,
    udp_socket: Option<Arc<UdpSocket>>,

    started: bool,
    starting: bool,
    first_started: bool,

    peers_connected: HashMap<Uuid, PeerConnected>,
    peer_uuid_to_session_uuid: HashMap<Uuid,Uuid>,

    udp_socket_receiver: UnboundedReceiver<Option<Arc<UdpSocket>>>,
    udp_socket_sender: Arc<UnboundedSender<Option<Arc<UdpSocket>>>>,

    connecting_downed_receiver: UnboundedReceiver<(Error,bool)>,
    connecting_downed_sender: Arc<UnboundedSender<(Error,bool)>>,

    peer_message_receiver: UnboundedReceiver<(Vec<u8>, Uuid, SocketAddr)>,
    peer_message_sender: Arc<UnboundedSender<(Vec<u8>, Uuid, SocketAddr)>>,
}

impl UdpServerSettings {
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;

        self
    }

    pub fn with_buffer_size(mut self, buffer_size: usize) -> Self {
        self.buffer_size = buffer_size;

        self
    }

    pub fn with_hook_udp_socket(mut self, hook: fn(tcp_stream: UdpSocket) -> UdpSocket) -> Self {
        self.hook_udp_socket = Some(hook);

        self
    }
}

impl Default for UdpServerSettings {
    fn default() -> Self {
        UdpServerSettings {
            address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8080,
            hook_udp_socket: None,
            buffer_size: 1024,
        }
    }
}

impl ServerSettingsPort for UdpServerSettings{
    fn create_port(self: Box<Self>) -> Box<dyn ServerPortTrait>{
        let (udp_socket_sender,udp_socket_receiver) = unbounded_channel::<Option<Arc<UdpSocket>>>();
        let (connecting_downed_sender,connecting_downed_receiver) = unbounded_channel::<(Error,bool)>();
        let (peer_message_sender,peer_message_receiver) = unbounded_channel::<(Vec<u8>, Uuid, SocketAddr)>();

        Box::new(UdpServerPort{
            settings: *self,
            udp_socket: None,

            started: false,
            starting: false,
            first_started: false,

            peers_connected: HashMap::new(),
            peer_uuid_to_session_uuid: HashMap::new(),

            udp_socket_receiver,
            udp_socket_sender: Arc::new(udp_socket_sender),

            connecting_downed_receiver,
            connecting_downed_sender: Arc::new(connecting_downed_sender),

            peer_message_receiver,
            peer_message_sender :Arc::new(peer_message_sender),
        })
    }
}

impl ServerPortTrait for UdpServerPort {
    fn start(&mut self, network_port_shared_infos: &dyn Any) {
        if self.started || self.starting { return; }

        self.starting = true;

        if let Some(default_network_port_shared_infos) = network_port_shared_infos.downcast_ref::<DefaultNetworkPortSharedInfosServer>()
            && let Some(runtime) = &default_network_port_shared_infos.get_runtime() {

            let settings = &self.settings;
            let address = (settings.address, settings.port);
            let hook_udp_socket = settings.hook_udp_socket;
            let udp_socket_sender = Arc::clone(&self.udp_socket_sender);
            let connecting_downed_sender = Arc::clone(&self.connecting_downed_sender);
            let peer_message_sender = Arc::clone(&self.peer_message_sender);
            let first_started = self.first_started;
            let buffer_size = settings.buffer_size;
            let udp_socket_actual = if let Some(udp_socket) = &self.udp_socket { Some(Arc::clone(udp_socket)) } else { None };

            runtime.spawn(async move {
                let was_empty = udp_socket_actual.is_none();

                let udp_socket = if let Some(udp_socket_actual) = udp_socket_actual { udp_socket_actual } else {
                    let udp_socket_future = UdpSocket::bind(address);

                     match udp_socket_future.await {
                        Ok(mut udp_socket_new) => {
                            udp_socket_new = match hook_udp_socket {
                                None => {
                                    udp_socket_new
                                }
                                Some(hook_udp_socket) => {
                                    hook_udp_socket(udp_socket_new)
                                }
                            };

                            Arc::new(udp_socket_new)
                        }
                        Err(e) => {
                            if let Err(send_error) = connecting_downed_sender.send((e,first_started)) {
                                warn!("Failed to send UDP port failed to connect, error: {}", send_error);
                            }

                            return;
                        }
                    }
                };

                if let Err(send_error) = udp_socket_sender.send(if was_empty { Some(Arc::clone(&udp_socket)) } else { None } ) {
                    warn!("Failed to send UDP port connected receiver, error: {}", send_error);
                    return;
                }

                let mut buf = vec![0u8; buffer_size];

                loop {
                    match udp_socket.recv_from(&mut buf).await {
                        Ok((len, addr)) => {
                            let buff: Vec<u8> = buf[..len].to_vec();

                            match extract_uuid(&buff) {
                                Ok((session_uuid,new_buffer)) => {
                                    if session_uuid.is_nil() {
                                        continue;
                                    }

                                    if let Err(send_error) = peer_message_sender.send((new_buffer, session_uuid, addr)) {
                                        warn!("Failed to send UDP peer message, error: {}", send_error);
                                    }
                                }
                                Err(_) => {
                                    continue;
                                }
                            };
                        }
                        Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                            if let Err(send_error) = connecting_downed_sender.send((e,first_started)) {
                                warn!("Failed to send UDP port disconnected, error: {}", send_error);
                                return;
                            }
                        }
                    }
                }
            });
        }
    }

    fn close(&mut self) {
        if let Some(udp_socket) = self.udp_socket.take() {
            drop(udp_socket);
        }
    }

    fn started(&mut self) -> (bool, bool) {
        if self.started {
            (true,false)
        }else {
            match self.udp_socket_receiver.try_recv() {
                Ok(udp_socket) => {
                    self.started = true;
                    self.starting = false;
                    self.first_started = true;

                    if let Some(udp_socket) = udp_socket {
                        self.udp_socket = Some(udp_socket);
                    }

                    (true,true)
                },
                Err(_) => {
                    (false,false)
                }
            }
        }
    }

    fn disconnected(&mut self) -> (bool, Option<Error>, bool) {
        match self.connecting_downed_receiver.try_recv() {
            Ok((error, first_started)) => {
                self.started = false;
                self.starting = false;

                (true,Some(error),first_started)
            },
            Err(_) => {
                (false,None,self.first_started)
            }
        }
    }

    fn get_peers_messages(&mut self) -> HashMap<Uuid, (Vec<Vec<u8>>, Option<Uuid>)> {
        let mut messages_list: HashMap<Uuid, (Vec<Vec<u8>>, Option<Uuid>)> = HashMap::new();
        let peers_connected = &mut self.peers_connected;
        let now = Instant::now();

        while let Ok((buff, session_uuid, socket)) = self.peer_message_receiver.try_recv() {
            if let Some(peer_connected) = peers_connected.get_mut(&session_uuid) {
                messages_list.insert(session_uuid, (Vec::from([buff]), peer_connected.peer_id));
            }else {
                peers_connected.insert(session_uuid, PeerConnected{
                    peer_id: None,
                    socket_addr: socket,
                    non_authenticated_instant: now,
                    last_pong_instant: now,
                    last_ping_instant: now,
                    is_local: false
                });

                messages_list.insert(session_uuid, (Vec::from([buff]), None));
            }
        }

        messages_list
    }

    fn get_port_reliability(&mut self) -> &PortReliability {
        &PortReliability::Unreliable
    }

    fn as_main_port(&mut self) -> bool {
        false
    }

    fn send_message_to_peer(&mut self, message_id: u32, peer_id: Uuid, network_port_shared_infos: &dyn Any, message: &dyn MessageTrait, _send_args: Option<&SendArgs>) {
        let peer_connected = if let Some(peer_connected) = self.peers_connected.get_mut(&peer_id) { Some(peer_connected) } else {
            if let Some(session_uuid) = self.peer_uuid_to_session_uuid.get_mut(&peer_id)
                && let Some(peer_connected) = self.peers_connected.get_mut(session_uuid)
            {
                Some(peer_connected)
            }else {
                None
            }
        };
        
        if let Some(udp_socket) = &self.udp_socket 
            && let Some(peer_connected) = peer_connected
            && let Some(default_network_port_shared_infos) = network_port_shared_infos.downcast_ref::<DefaultNetworkPortSharedInfosServer>()
            && let Some(runtime) = &default_network_port_shared_infos.get_runtime()
        {
            let message_infos = &MessageInfos{
                message_id,
                message: postcard::to_stdvec(message).unwrap(),
            };

            let buffer = match postcard::to_stdvec(message_infos) {
                Ok(buff) => {buff}
                Err(_) => {
                    warn!("Error to serialize message");
                    return;
                }
            };

            let socket_addr = peer_connected.socket_addr;
            let udp_socket = Arc::clone(udp_socket);

            runtime.spawn(async move {
                udp_socket.send_to(&buffer, socket_addr).await.ok()
            });
        }
    }

    fn is_main_port(&self) -> bool {
        false
    }

    fn get_anonymous_sessions(&self) -> Vec<Uuid> {
        let mut annoy_anonymous_sessions = Vec::new();

        for (session_uuid,peer_connected) in self.peers_connected.iter() {
            if peer_connected.peer_id.is_none(){
                annoy_anonymous_sessions.push(*session_uuid);
            }
        }

        annoy_anonymous_sessions
    }

    fn get_authenticated_sessions(&self) -> Vec<(Uuid,Uuid)> {
        let mut annoy_anonymous_sessions = Vec::new();

        for (session_uuid,peer_connected) in self.peers_connected.iter() {
            if let Some(peer_id) = peer_connected.peer_id {
                annoy_anonymous_sessions.push((*session_uuid,peer_id));
            }
        }

        annoy_anonymous_sessions
    }

    fn get_all_sessions(&self) -> Vec<(Uuid, Option<Uuid>)> {
        let mut all_sessions: Vec<(Uuid,Option<Uuid>)> = Vec::new();

        for (session_uuid,peer_connected) in self.peers_connected.iter() {
            if let Some(peer_id) = peer_connected.peer_id {
                all_sessions.push((*session_uuid,Some(peer_id)));
            }else {
                all_sessions.push((*session_uuid,None));
            }
        }

        all_sessions
    }

    fn get_port_status(&self) -> PortStatus {
        PortStatus {
            started: self.started,
            first_started: self.first_started,
            starting: self.starting
        }
    }

    fn send_message_to_all_peer(&mut self, message_id: u32, local_peer_uuid_option: &Option<Uuid>, network_port_shared_infos: &dyn Any, message: &dyn MessageTrait, _send_args: Option<&SendArgs>, just_authenticated: bool, exceptions: &Vec<Uuid>) {
        if let Some(udp_socket) = &self.udp_socket && let Some(default_network_port_shared_infos) = network_port_shared_infos.downcast_ref::<DefaultNetworkPortSharedInfosServer>()
            && let Some(runtime) = &default_network_port_shared_infos.get_runtime() {

            let message_infos = &MessageInfos{
                message_id,
                message: postcard::to_stdvec(message).unwrap(),
            };

            let buffer = match postcard::to_stdvec(message_infos) {
                Ok(buff) => {buff}
                Err(_) => {
                    warn!("Error to serialize message");
                    return;
                }
            };

            for (peer_uuid,peer_connected) in self.peers_connected.iter_mut() {
                if exceptions.contains(peer_uuid) || (just_authenticated && peer_connected.peer_id.is_none()) {
                    continue;
                }

                if let Some(local_peer_uuid) = local_peer_uuid_option && local_peer_uuid == peer_uuid {
                    continue;
                }

                let socket_addr = peer_connected.socket_addr;
                let udp_socket = Arc::clone(udp_socket);
                let buffer_clone = buffer.clone();

                runtime.spawn(async move {
                    udp_socket.send_to(&buffer_clone, socket_addr).await.ok()
                });
            }
        }
    }

    fn get_peers_disconnected(&mut self) -> HashMap<Uuid,(Option<Uuid>, Error, bool)> {
        let now = Instant::now();
        let mut disconnected_list: HashMap<Uuid,(Option<Uuid>, Error, bool)> = HashMap::new();

        self.peers_connected.retain(|session_uuid, peer_connected| {
            if peer_connected.peer_id.is_none()
            && now.duration_since(peer_connected.non_authenticated_instant) >= Duration::from_secs(120)
            {
                disconnected_list.insert(*session_uuid,(peer_connected.peer_id, Error::from(ErrorKind::TimedOut), false));
                return false
            }

            if now.duration_since(peer_connected.last_pong_instant) >= Duration::from_secs(120) {
                disconnected_list.insert(*session_uuid,(peer_connected.peer_id, Error::from(ErrorKind::TimedOut), false));
                return false
            }

            true
        });

        disconnected_list
    }

    fn authenticate_peer(&mut self, current_session_uuid: Uuid, new_peer_id: Uuid, new_session_uuid: Option<Uuid>, is_local: bool) {
        if let Some(peer_connected) = self.peers_connected.get_mut(&current_session_uuid) {
            peer_connected.peer_id = Some(new_peer_id);

            if let Some(new_session_uuid) = new_session_uuid {
                let mut peer_connected = self.peers_connected.remove(&current_session_uuid).unwrap();

                peer_connected.is_local = is_local;

                self.peers_connected.insert(new_session_uuid, peer_connected);
                self.peer_uuid_to_session_uuid.insert(new_peer_id, new_session_uuid);
            } else {
                self.peer_uuid_to_session_uuid.insert(new_peer_id, current_session_uuid);
            }
        }
    }

    fn is_session_authenticated(&self, session_uuid: &Uuid) -> bool {
        if let Some(peer_connected) = self.peers_connected.get(session_uuid) {
            return peer_connected.peer_id.is_some()
        }

        false
    }

    fn disconnect_peer_or_session(&mut self, uuid: &Uuid) -> Option<(Uuid,Option<Uuid>)> {
        if let Some(peer_connected) = self.peers_connected.remove(uuid) {
            let peer_id = peer_connected.peer_id;
            return Some((*uuid, peer_id))
        }

        None
    }

    fn ping(&mut self, session_uuid: &Uuid, network_port_shared_infos: &dyn Any) {
        let instant = Instant::now();

        if let Some(peer_connected) =self.peers_connected.get_mut(session_uuid)
            && instant.duration_since(peer_connected.last_ping_instant) >= Duration::from_secs(10)
            && let Some(default_network_port_shared_infos) = network_port_shared_infos.downcast_ref::<DefaultNetworkPortSharedInfosServer>()
            && let Some(runtime) = &default_network_port_shared_infos.get_runtime()
            && let Some(udp_socket) = &self.udp_socket
        {
            peer_connected.last_ping_instant = instant;

            let udp_socket = Arc::clone(udp_socket);
            let socket_addr = peer_connected.socket_addr;

            let buffer = match postcard::to_stdvec("ping") {
                Ok(buff) => {buff}
                Err(_) => {
                    warn!("Error to serialize message");
                    return;
                }
            };

            runtime.spawn(async move {
                udp_socket.send_to(&buffer, socket_addr).await.ok()
            });
        }
    }

    fn pong(&mut self, session_uuid: &Uuid, bytes: &[u8], _network_port_shared_infos: Option<&dyn Any>) {
        if let Ok(msg) = postcard::from_bytes::<String>(bytes)
        && msg == "ping" && let Some(peer_connected) = self.peers_connected.get_mut(session_uuid)
        {
            peer_connected.last_pong_instant = Instant::now();
        }
    }
}

