#![cfg(target_arch = "wasm32")]
use std::any::Any;
use std::cell::RefCell;
use std::io::{Error, ErrorKind};
use std::rc::Rc;
use bevy::asset::uuid::Uuid;
use bevy::log::{error, warn};
use bevy::platform::exports::wasm_bindgen_futures::spawn_local;
use gloo_net::websocket::futures::WebSocket;
use gloo_net::websocket::{Message};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use send_wrapper::SendWrapper;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use crate::shared::plugins::messaging::{MessageInfos, MessageTrait, SendArgs};
use crate::shared::plugins::network::{ClientPortTrait, ClientSettingsPort, PortReliability};

pub struct WasmWebSocketClientSettings{
    url: String,
    hook_web_socket: Option<fn(web_socket: WebSocket) -> WebSocket>
}

pub struct WasmWebSocketClientPort{
    settings: WasmWebSocketClientSettings,

    started: bool,
    starting: bool,
    first_started: bool,
    main_port: bool,
    authenticated: bool,
    listening_server: bool,
    sending_server_message: bool,

    split_sink: Option<SendWrapper<Rc<RefCell<SplitSink<WebSocket, Message>>>>>,
    split_stream: Option<SendWrapper<Rc<RefCell<SplitStream<WebSocket>>>>>,

    web_socket_receiver: SendWrapper<UnboundedReceiver<WebSocket>>,
    web_socket_sender: SendWrapper<Rc<UnboundedSender<WebSocket>>>,

    message_receiver: UnboundedReceiver<Vec<u8>>,
    message_sender: SendWrapper<Rc<UnboundedSender<Vec<u8>>>>,

    send_message_queue_receiver: SendWrapper<Rc<RefCell<UnboundedReceiver<Vec<u8>>>>>,
    send_message_queue_sender: UnboundedSender<Vec<u8>>,

    stop_send_message_receiver: SendWrapper<Rc<RefCell<UnboundedReceiver<()>>>>,
    stop_send_message_sender: UnboundedSender<()>,

    connecting_downed_receiver: UnboundedReceiver<(Error,bool)>,
    connecting_downed_sender: SendWrapper<Rc<UnboundedSender<(Error,bool)>>>,
}

impl WasmWebSocketClientSettings {
    pub fn with_web_socket_hook(mut self, hook_web_socket: fn(web_socket: WebSocket) -> WebSocket) -> Self{
        self.hook_web_socket = Some(hook_web_socket);

        self
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = url;

        self
    }
}

impl Default for WasmWebSocketClientSettings{
    fn default() -> Self {
        WasmWebSocketClientSettings {
            url: "ws://127.0.0.1:1234".to_string(),
            hook_web_socket: None
        }
    }
}

impl ClientSettingsPort for WasmWebSocketClientSettings{
    fn create_port(self: Box<Self>) -> Box<dyn ClientPortTrait> {
        let (web_socket_sender,web_socket_receiver) = unbounded_channel::<WebSocket>();
        let (message_sender,message_receiver) = unbounded_channel::<Vec<u8>>();
        let (send_message_queue_sender,send_message_queue_receiver) = unbounded_channel::<Vec<u8>>();
        let (stop_send_message_sender,stop_send_message_receiver) = unbounded_channel::<()>();
        let (connecting_downed_sender,connecting_downed_receiver) = unbounded_channel::<(Error,bool)>();

        Box::new(WasmWebSocketClientPort{
            settings: *self,

            started: false,
            starting: false,
            first_started: false,
            main_port: false,
            authenticated: false,
            listening_server: false,
            sending_server_message: false,

            split_sink: None,
            split_stream: None,

            web_socket_receiver: SendWrapper::new(web_socket_receiver),
            web_socket_sender: SendWrapper::new(Rc::new(web_socket_sender)),

            send_message_queue_receiver: SendWrapper::new(Rc::new(RefCell::new(send_message_queue_receiver))),
            send_message_queue_sender,

            stop_send_message_receiver: SendWrapper::new(Rc::new(RefCell::new(stop_send_message_receiver))),
            stop_send_message_sender,

            message_receiver,
            message_sender: SendWrapper::new(Rc::new(message_sender)),

            connecting_downed_receiver,
            connecting_downed_sender: SendWrapper::new(Rc::new(connecting_downed_sender)),
        })
    }
}

impl ClientPortTrait for WasmWebSocketClientPort{
    fn start(&mut self, _network_port_shared_infos: &dyn Any) {
        if self.started || self.starting { return; }

        self.starting = true;
        self.listening_server = false;
        self.sending_server_message = false;

        let settings = &self.settings;
        let connecting_downed_sender = Rc::clone(&self.connecting_downed_sender);
        let web_socket_sender = Rc::clone(&self.web_socket_sender);
        let url = settings.url.clone();
        let first_started = self.first_started;
        let hook_web_socket = settings.hook_web_socket;

        spawn_local(async move {
            let web_socket = WebSocket::open(&url);

            match web_socket {
                Ok(mut web_socket) => {
                    match hook_web_socket {
                        Some(hook_web_socket) => {
                            web_socket = hook_web_socket(web_socket);
                        },
                        _ => {}
                    }

                    if let Err(send_error) = web_socket_sender.send(web_socket){
                        warn!("Failed to send WASM Websocket client connected, error: {}", send_error);
                    }
                }
                Err(e) => {
                    if let Err(send_error) = connecting_downed_sender.send((Error::new(ErrorKind::NotConnected, e), first_started)) {
                        warn!("Failed to send WASM Websocket port failed to connect, error: {}", send_error);
                    }
                }
            }
        });
    }

    fn close(&mut self) {
        if let Some(split_sink) = self.split_sink.take() {
            drop(split_sink);
        }
        
        if let Some(split_stream) = self.split_stream.take() {
            drop(split_stream);
        }
    }

    fn started(&mut self) -> (bool, bool) {
        if self.started {
            (true,false)
        }else {
            match self.web_socket_receiver.try_recv() {
                Ok(web_socket) => {
                    self.started = true;
                    self.starting = false;
                    self.first_started = true;
                    
                    let (sink, stream) = web_socket.split();

                    self.split_sink = Some(SendWrapper::new(Rc::new(RefCell::new(sink))));
                    self.split_stream = Some(SendWrapper::new(Rc::new(RefCell::new(stream))));
                    
                    (true,true)
                }
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
                self.listening_server = false;
                self.sending_server_message = false;
                self.authenticated = false;

                if let Err(send_error) = self.stop_send_message_sender.send(()) {
                    warn!("Failed to send WASM Websocket client disconnected with error: {}", send_error);
                }

                self.close();

                (true,Some(error),first_started)
            },
            Err(_) => {
                (false,None,self.first_started)
            }
        }
    }

    fn get_server_messages(&mut self) -> Vec<Vec<u8>> {
        let mut messages: Vec<Vec<u8>> = Vec::new();

        loop {
            match self.message_receiver.try_recv() {
                Ok(bytes) => {
                    messages.push(bytes);
                }
                Err(_) => {
                    break;
                }
            }
        }

        messages
    }

    fn get_port_reliability(&mut self) -> &PortReliability {
        &PortReliability::Reliable
    }

    fn as_main_port(&mut self) -> bool {
        self.main_port = true;

        true
    }

    fn send_message_for_server(&mut self, message_id: u32, _network_port_shared_infos: &dyn Any, message: &dyn MessageTrait, _local_session_uuid: Option<Uuid>, _send_args: Option<&SendArgs>) {
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

        if let Err(send_error) =  self.send_message_queue_sender.send(buffer) {
            warn!("Failed to send message for server on WASM Websocket client, error: {}", send_error);
            return;
        }

        if !self.sending_server_message && let Some(split_sink) = &self.split_sink {
            self.sending_server_message = true;

            let send_message_queue_receiver = Rc::clone(&self.send_message_queue_receiver);
            let stop_send_message_receiver = Rc::clone(&self.stop_send_message_receiver);
            let split_sink = Rc::clone(split_sink);

            spawn_local(async move {
                let mut split_sink_borrow = split_sink.borrow_mut();
                let mut send_message_queue_receiver_borrow = send_message_queue_receiver.borrow_mut();
                let mut stop_send_message_receiver = stop_send_message_receiver.borrow_mut();

                loop {
                    if let Ok(_) = stop_send_message_receiver.try_recv() {
                        return;
                    }

                    match send_message_queue_receiver_borrow.try_recv() {
                        Ok(buffer) => {
                            if let Err(send_error) = split_sink_borrow.send(Message::Bytes(buffer)).await {
                                warn!("Failed to send message bytes for server on WASM Websocket client, error: {}", send_error)
                            }
                        }
                        Err(_) => {
                            continue;
                        }
                    }
                }
            });
        }
    }

    fn is_main_port(&self) -> bool {
        self.main_port
    }

    fn listen_to_server(&mut self, _network_port_shared_infos: &dyn Any) {
        if self.listening_server { return; }

        if let Some(split_stream) = &self.split_stream {
            self.listening_server = true;

            let split_stream = Rc::clone(split_stream);
            let connecting_downed_sender = Rc::clone(&self.connecting_downed_sender);
            let first_started = self.first_started;
            let message_sender = Rc::clone(&self.message_sender);

            spawn_local(async move {
                let mut split_stream_mut = split_stream.borrow_mut();

                loop {
                    match split_stream_mut.next().await {
                        Some(msg) => {
                            match msg {
                                Ok(Message::Bytes(bytes)) => {
                                    if let Err(send_error) = message_sender.send(bytes) {
                                        warn!("Failed to send message bytes on WASM Websocket client, error: {}", send_error)
                                    }
                                }
                                Ok(Message::Text(text)) => {
                                    warn!("Unexpected text : {}", text);
                                }
                                Err(err) => {
                                    error!("Error reading message: {:?}", err);

                                    if let Err(send_error) = connecting_downed_sender.send((Error::new(ErrorKind::NotConnected, err), first_started)) {
                                        warn!("Failed to send WASM Websocket port connection closed, error: {}", send_error);
                                    }

                                    return;
                                }
                            }
                        }
                        None => {
                            if let Err(send_error) = connecting_downed_sender.send((Error::new(ErrorKind::NotConnected, "Connection closed"), first_started)) {
                                warn!("Failed to send WASM Websocket port connection closed, error: {}", send_error);
                            }

                            return;
                        }
                    }
                }
            })
        }
    }

    fn authenticate_port(&mut self) {
        self.authenticated = true;
    }

    fn is_port_authenticated(&self) -> bool {
        self.authenticated
    }
}