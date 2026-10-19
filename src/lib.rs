#![no_std]
#![warn(missing_docs)]

//! Protocolo de comunicación serial basado en COBS/R y CRC.

mod codec;

pub use codec::{
    ProtocolErrors, decode, encode, min_decode_output_buffer_size, min_encode_output_buffer_size,
};
