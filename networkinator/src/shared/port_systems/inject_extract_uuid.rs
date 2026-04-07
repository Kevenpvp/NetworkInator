use std::io::{Error, ErrorKind};
use bevy::asset::uuid::Uuid;

pub fn inject_uuid(bytes: Vec<u8>, uuid: Uuid) -> Vec<u8> {
    let mut new_bytes = Vec::with_capacity(16 + bytes.len());
    new_bytes.extend_from_slice(uuid.as_bytes()); // UUID primeiro
    new_bytes.extend_from_slice(&bytes);          // depois o payload
    new_bytes
}

pub fn extract_uuid(buf: &[u8]) -> Result<(Uuid, Vec<u8>), Error> {
    if buf.len() < 16 {
        return Err(Error::new(ErrorKind::InvalidData, "Peer didnt send a valid uuid on top"));
    }
    
    let uuid = Uuid::from_slice(&buf[..16])
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    
    let payload = buf[16..].to_vec();

    Ok((uuid, payload))
}