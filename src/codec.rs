//! Codificación y decodificación de tramas del protocolo serial.

use cobs2::cobsr;
use crc::Crc;

type CrcType = u16;
const CRC_ALGORITHM: &crc::Algorithm<CrcType> = &crc::CRC_16_IBM_3740;
const CRC_CALCULATOR: Crc<CrcType> = Crc::<CrcType>::new(CRC_ALGORITHM);
const CRC_SIZE_BYTES: usize = size_of::<CrcType>();

type DelimiterType = u8;
const DELIMITER: DelimiterType = 0x00;
const DELIMITER_SIZE: usize = size_of::<DelimiterType>();

const MIN_INPUT_BUFFER_SIZE: usize = CRC_SIZE_BYTES + DELIMITER_SIZE;

/// Tipos de error posibles en la codificación y decodificación
///
/// # Variants
///
/// - `InputBufferTooSmall` - Buffer de entrada demasiado pequeño, se consultan con sus funciones designadas
/// - `OutputBufferTooSmall` - Buffer de salida demasiado pequeño, se consultan con sus funciones designadas
/// - `CrcMismatch` - El CRC declarado en el buffer de entrada no coincide con el CRC calculado a partir del mismo payload
/// - `InputWithoutDelimiter` - El buffer de entrada no cuenta con el delimitador
pub enum ProtocolErrors {
    /// Buffer de entrada demasiado pequeño, se consultan con sus funciones designadas
    InputBufferTooSmall,

    /// Buffer de salida demasiado pequeño, se consultan con sus funciones designadas
    OutputBufferTooSmall,

    /// El CRC declarado en el buffer de entrada no coincide con el CRC calculado a partir del mismo payload
    CrcMismatch,

    /// El buffer de entrada no cuenta con el delimitador
    InputWithoutDelimiter,
}

/// Codifica un arreglo de bytes al protocolo de transmisión serial.
///
/// # Arguments
///
/// - `input` (`& [u8]`) - Buffer de bytes a codificar.
/// - `output` (`&'a mut [u8]`) - Buffer de bytes en donde se escribirá la salida.
///
/// # Returns
///
/// - `Result<&'a [u8], ProtocolErrors>` - Slice que contiene la salida, vive en output.
///
/// # Errors
///
/// Devuelve [`ProtocolErrors::OutputBufferTooSmall`] cuando el buffer de salida no cumple con el tamaño mínimo requerido,
/// dicho tamaño puede ser consultado con [`min_encode_output_buffer_size`].
///
/// # Examples
///
/// ```
/// use serial_protocol::encode;
///
/// let mut output_buffer = [0u8; 10];
///
/// let output_slice = match encode(b"011", &mut output_buffer) {
///     Ok(output_slice) => output_slice,
///     Err(_) => panic!("the output buffer should be large enough"),
/// };
///
/// assert_eq!(output_slice.last(), Some(&0));
/// ```
pub fn encode<'a>(input: &[u8], output: &'a mut [u8]) -> Result<&'a [u8], ProtocolErrors> {
    if output.len() < min_encode_output_buffer_size(input.len()) {
        return Err(ProtocolErrors::OutputBufferTooSmall);
    }

    let calculated_crc = CRC_CALCULATOR.checksum(input).to_be_bytes();
    let input_bytes = input.iter().copied().chain(calculated_crc);
    let encoded = cobsr::encode_iter(input_bytes).chain([DELIMITER]);

    let bytes_written = fill_buffer_with_iterator(output, encoded)?;

    Ok(&output[..bytes_written])
}

/// Decodifica un buffer previamente codificado con [`encode`] y lo coloca en un buffer de salida
///
/// # Arguments
///
/// - `input` (`&[u8]`) - Buffer codificado.
/// - `output` (`&'a mut [u8]`) - Buffer donde se colocará la salida decodificada
///
/// # Returns
///
/// - `Result<&'a [u8], ProtocolErrors>` - Slice donde se localiza la salida, vive en output.
///
/// # Errors
///
/// Devuelve [`ProtocolErrors::InputBufferTooSmall`] cuando el buffer de entrada tiene un tamaño menor al mínimo del protocolo
/// Devuelve [`ProtocolErrors::InputWithoutDelimiter`] cuando el buffer de entrada no cuenta con el delimitador
/// Devuelve [`ProtocolErrors::OutputBufferTooSmall`] cuando el buffer de salida no tiene el tamaño mínimo para colocar el output,
/// dicho tamaño puede ser consultado con [`min_decode_output_buffer_size`]
///
/// # Examples
///
/// ```
/// use serial_protocol::{decode, encode};
///
/// let input = b"011";
/// let mut encoded_buffer = [0u8; 10];
/// let encoded = match encode(input, &mut encoded_buffer) {
///     Ok(encoded) => encoded,
///     Err(_) => panic!("the encoded buffer should be large enough"),
/// };
///
/// let mut decoded_buffer = [0u8; 5];
/// let decoded = match decode(encoded, &mut decoded_buffer) {
///     Ok(decoded) => decoded,
///     Err(_) => panic!("a frame produced by encode should be valid"),
/// };
///
/// assert_eq!(decoded, input);
/// ```
pub fn decode<'a>(input: &[u8], output: &'a mut [u8]) -> Result<&'a [u8], ProtocolErrors> {
    if input.len() < MIN_INPUT_BUFFER_SIZE {
        return Err(ProtocolErrors::InputBufferTooSmall);
    }

    if *input.last().unwrap() != DELIMITER {
        return Err(ProtocolErrors::InputWithoutDelimiter);
    }

    let input_without_delimiter = input[..input.len() - DELIMITER_SIZE].iter().copied();
    let decoded = cobsr::decode_iter(input_without_delimiter);

    let bytes_written = fill_buffer_with_iterator(output, decoded)?;

    verify_crc(&output[..bytes_written])?;

    Ok(&output[..(bytes_written - CRC_SIZE_BYTES)])
}

/// Devuelve el tamaño mínimo que debe tener el buffer de salida colocado en [`encode`]
///
/// # Arguments
///
/// - `lenght_to_encode` (`usize`) - Tamaño del buffer a codificar
///
/// # Returns
///
/// - `usize` - Tamaño mínimo que debe tener el buffer de salida
///
/// # Examples
///
/// ```
/// use serial_protocol::min_encode_output_buffer_size;
///
/// const OUTPUT_SIZE: usize = min_encode_output_buffer_size(3);
/// let output_buffer = [0u8; OUTPUT_SIZE];
///
/// assert_eq!(output_buffer.len(), OUTPUT_SIZE);
/// ```
pub const fn min_encode_output_buffer_size(lenght_to_encode: usize) -> usize {
    cobsr::encode_max_output_size(lenght_to_encode + CRC_SIZE_BYTES) + DELIMITER_SIZE
}

/// Devuelve el tamaño mínimo que debe tener el buffer de salida colocado en [`decode`]
///
/// # Arguments
///
/// - `lenght_to_decode` (`usize`) - Tamaño del buffer a decodificar
///
/// # Returns
///
/// - `usize` - Tamaño mínimo del buffer de salida
///
/// # Examples
///
/// ```
/// use serial_protocol::min_decode_output_buffer_size;
///
/// const OUTPUT_SIZE: usize = min_decode_output_buffer_size(10);
/// let output_buffer = [0u8; OUTPUT_SIZE];
///
/// assert_eq!(output_buffer.len(), OUTPUT_SIZE);
/// ```
pub const fn min_decode_output_buffer_size(lenght_to_decode: usize) -> usize {
    cobsr::decode_max_output_size(lenght_to_decode)
}

fn verify_crc(buffer_with_crc: &[u8]) -> Result<(), ProtocolErrors> {
    let payload_lenght = buffer_with_crc
        .len()
        .checked_sub(CRC_SIZE_BYTES)
        .ok_or(ProtocolErrors::InputBufferTooSmall)?;

    let (payload, declared_crc) = buffer_with_crc.split_at(payload_lenght);

    let calculated_crc = CRC_CALCULATOR.checksum(payload).to_be_bytes();

    if calculated_crc == declared_crc {
        Ok(())
    } else {
        Err(ProtocolErrors::CrcMismatch)
    }
}

fn fill_buffer_with_iterator(
    buffer_to_fill: &mut [u8],
    iterator: impl Iterator<Item = u8>,
) -> Result<usize, ProtocolErrors> {
    let mut bytes_written = 0;

    for byte in iterator {
        let slot = buffer_to_fill
            .get_mut(bytes_written)
            .ok_or(ProtocolErrors::OutputBufferTooSmall)?;

        *slot = byte;
        bytes_written += 1;
    }

    Ok(bytes_written)
}
