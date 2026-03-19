use bevy::prelude::{Res, ResMut};

pub mod plugins;
pub mod port_systems;

#[cfg(target_arch = "wasm32")]
pub type NetResMut<'a, T> = NonSendMut<'a, T>;

#[cfg(target_arch = "wasm32")]
pub type NetRes<'a, T> = NonSend<'a, T>;

#[cfg(not(target_arch = "wasm32"))]
pub type NetResMut<'a, T> = ResMut<'a, T>;

#[cfg(not(target_arch = "wasm32"))]
pub type NetRes<'a, T> = Res<'a, T>;
