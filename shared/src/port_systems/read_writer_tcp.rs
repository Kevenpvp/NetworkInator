use tokio::net::tcp::{OwnedWriteHalf};
use std::io::Error;
use tokio::io::{AsyncWriteExt};

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum OrderOptions{
    LittleEndian,
    BigEndian
}

#[derive(Debug, Clone, Copy)]
pub enum BytesOptions {
    U8,
    U16,
    U32,
    U64,
    U128,

    I8,
    I16,
    I32,
    I64,
    I128,

    F32,
    F64,
}

#[derive(Debug)]
pub enum ReadValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),

    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),

    F32(f32),
    F64(f64),
}

pub fn read_value_to_usize(value: ReadValue) -> usize {
    match value {
        // Unsigned
        ReadValue::U8(v) => v as usize,
        ReadValue::U16(v) => v as usize,
        ReadValue::U32(v) => v as usize,
        ReadValue::U64(v) => v as usize,
        ReadValue::U128(v) => v as usize,

        // Signed
        ReadValue::I8(v) => v as usize,
        ReadValue::I16(v) => v as usize,
        ReadValue::I32(v) => v as usize,
        ReadValue::I64(v) => v as usize,
        ReadValue::I128(v) => v as usize,

        // Floats
        ReadValue::F32(v) => v as usize,
        ReadValue::F64(v) => v as usize,
    }
}

pub fn value_from_number(number: f64, bytes: BytesOptions) -> ReadValue {
    match bytes {
        // Unsigned
        BytesOptions::U8 => ReadValue::U8(number as u8),
        BytesOptions::U16 => ReadValue::U16(number as u16),
        BytesOptions::U32 => ReadValue::U32(number as u32),
        BytesOptions::U64 => ReadValue::U64(number as u64),
        BytesOptions::U128 => ReadValue::U128(number as u128),

        // Signed
        BytesOptions::I8 => ReadValue::I8(number as i8),
        BytesOptions::I16 => ReadValue::I16(number as i16),
        BytesOptions::I32 => ReadValue::I32(number as i32),
        BytesOptions::I64 => ReadValue::I64(number as i64),
        BytesOptions::I128 => ReadValue::I128(number as i128),

        // Floats
        BytesOptions::F32 => ReadValue::F32(number as f32),
        BytesOptions::F64 => ReadValue::F64(number),
    }
}

pub async fn write_from_settings(
    write_half: &mut OwnedWriteHalf,
    value: &ReadValue,
    order: &OrderOptions,
) -> Result<(), Error> {
    match value {
        // Unsigned
        ReadValue::U8(v) => write_half.write_u8(*v).await?,
        ReadValue::U16(v) => match order {
            OrderOptions::LittleEndian => write_half.write_u16_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_u16(*v).await?,
        },
        ReadValue::U32(v) => match order {
            OrderOptions::LittleEndian => write_half.write_u32_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_u32(*v).await?,
        },
        ReadValue::U64(v) => match order {
            OrderOptions::LittleEndian => write_half.write_u64_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_u64(*v).await?,
        },
        ReadValue::U128(v) => match order {
            OrderOptions::LittleEndian => write_half.write_u128_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_u128(*v).await?,
        },

        // Signed
        ReadValue::I8(v) => write_half.write_i8(*v).await?,
        ReadValue::I16(v) => match order {
            OrderOptions::LittleEndian => write_half.write_i16_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_i16(*v).await?,
        },
        ReadValue::I32(v) => match order {
            OrderOptions::LittleEndian => write_half.write_i32_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_i32(*v).await?,
        },
        ReadValue::I64(v) => match order {
            OrderOptions::LittleEndian => write_half.write_i64_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_i64(*v).await?,
        },
        ReadValue::I128(v) => match order {
            OrderOptions::LittleEndian => write_half.write_i128_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_i128(*v).await?,
        },

        // Floats
        ReadValue::F32(v) => match order {
            OrderOptions::LittleEndian => write_half.write_f32_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_f32(*v).await?,
        },
        ReadValue::F64(v) => match order {
            OrderOptions::LittleEndian => write_half.write_f64_le(*v).await?,
            OrderOptions::BigEndian => write_half.write_f64(*v).await?,
        },
    }

    Ok(())
}

pub fn get_bytes_size(options: &BytesOptions) -> usize {
    match options {
        BytesOptions::U8 | BytesOptions::I8 => 1,
        BytesOptions::U16 | BytesOptions::I16 => 2,
        BytesOptions::U32 | BytesOptions::I32 | BytesOptions::F32 => 4,
        BytesOptions::U64 | BytesOptions::I64 | BytesOptions::F64 => 8,
        BytesOptions::U128 | BytesOptions::I128 => 16,
    }
}

fn parse_value_from_bytes(bytes: &[u8], opt: &BytesOptions, order: &OrderOptions) -> ReadValue {
    match opt {
        BytesOptions::U8 => ReadValue::U8(bytes[0]),
        BytesOptions::U16 => {
            let v = if *order == OrderOptions::LittleEndian {
                u16::from_le_bytes(bytes.try_into().unwrap())
            } else {
                u16::from_be_bytes(bytes.try_into().unwrap())
            };
            ReadValue::U16(v)
        }
        BytesOptions::U32 => {
            let v = if *order == OrderOptions::LittleEndian {
                u32::from_le_bytes(bytes.try_into().unwrap())
            } else {
                u32::from_be_bytes(bytes.try_into().unwrap())
            };
            ReadValue::U32(v)
        }
        // ... Repetir o padrão para U64, U128, I-types e F-types ...
        _ => todo!("Implementar os outros tipos de BytesOptions conforme necessário"),
    }
}

pub fn extract_messages_from_buffer(
    buffer: &mut Vec<u8>,
    bytes_opt: &BytesOptions,
    order: &OrderOptions,
) -> Vec<Vec<u8>> {
    let mut messages = Vec::new();
    let header_size = get_bytes_size(bytes_opt);

    loop {
        // Se não tem bytes suficientes nem para ler o tamanho da próxima mensagem, para.
        if buffer.len() < header_size {
            break;
        }

        // 1. Ler o tamanho prefixado sem remover do buffer ainda
        let msg_len = {
            let header_bytes = &buffer[..header_size];
            let read_val = parse_value_from_bytes(header_bytes, bytes_opt, order);
            read_value_to_usize(read_val)
        };

        // 2. Verificar se a mensagem inteira já está no buffer
        // O tamanho total ocupado é o header + o corpo da mensagem
        if buffer.len() >= header_size + msg_len {
            // Remove o header do buffer
            buffer.drain(..header_size);
            // Remove e coleta a mensagem
            let message = buffer.drain(..msg_len).collect();
            messages.push(message);
        } else {
            // Mensagem incompleta, sai do loop e espera mais dados
            break;
        }
    }

    messages
}