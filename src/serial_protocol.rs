use cobs2::cobsr;
use crc::Crc;

type CrcType = u16;
const CRC_ALGORITHM: &crc::Algorithm<CrcType> = &crc::CRC_16_IBM_3740;
const CRC_CALCULATOR: Crc<CrcType> = Crc::<CrcType>::new(CRC_ALGORITHM);
const CRC_SIZE_BYTES: usize = 2; // TODO: Podría aprovechar el campo widht de CRC_ALGORITHM para calcularlo

type DelimiterType = u8;
const DELIMITER: DelimiterType = 0x00;
const DELIMITER_SIZE: usize = size_of::<DelimiterType>();

const MIN_INPUT_BUFFER_SIZE: usize = CRC_SIZE_BYTES + DELIMITER_SIZE;

pub enum ProtocolErrors {
    InputBufferTooSmall,
    OutputBufferTooSmall,
    CrcMismatch,
    InputWithoutDelimiter,
}

pub fn encode(input: &[u8], output: &mut [u8]) -> Result<usize, ProtocolErrors> {
    if output.len() < min_encode_output_buffer_size(input.len()) {
        return Err(ProtocolErrors::OutputBufferTooSmall);
    }

    let calculated_crc = CRC_CALCULATOR.checksum(input).to_be_bytes();
    let input_bytes = input.iter().copied().chain(calculated_crc);
    let encoded = cobsr::encode_iter(input_bytes);

    let bytes_written = fill_buffer_with_iterator(output, encoded)?;

    // TODO: Podría usar chain en el encoded para que el delimitador se coloque en la misma operación anterior?
    output[bytes_written] = DELIMITER;

    Ok(bytes_written + DELIMITER_SIZE)
}

pub fn decode(input: &[u8], output: &mut [u8]) -> Result<usize, ProtocolErrors> {
    if input.len() < MIN_INPUT_BUFFER_SIZE {
        return Err(ProtocolErrors::InputBufferTooSmall);
    }

    if *input.last().unwrap() != DELIMITER {
        return Err(ProtocolErrors::InputWithoutDelimiter);
    }

    let input_without_delimiter = input[0..input.len() - DELIMITER_SIZE].iter().copied();
    let decoded = cobsr::decode_iter(input_without_delimiter);

    let bytes_written = fill_buffer_with_iterator(output, decoded)?;

    verify_crc(&output[0..bytes_written])?;

    Ok(bytes_written - CRC_SIZE_BYTES)
}

pub const fn min_encode_output_buffer_size(lenght_to_encode: usize) -> usize {
    cobsr::encode_max_output_size(lenght_to_encode + CRC_SIZE_BYTES) + DELIMITER_SIZE
}

pub const fn min_decode_output_buffer_size(lenght_to_decode: usize) -> usize {
    cobsr::decode_max_output_size(lenght_to_decode)
}

fn verify_crc(buffer_with_crc : &[u8]) -> Result<(), ProtocolErrors> {
    let payload_lenght = buffer_with_crc.len().checked_sub(CRC_SIZE_BYTES).ok_or(ProtocolErrors::InputBufferTooSmall)?;

    let (payload, declared_crc) = buffer_with_crc.split_at(payload_lenght);
    
    let calculated_crc = CRC_CALCULATOR.checksum(payload).to_be_bytes();

    if calculated_crc == declared_crc {
        Ok(())
    }
    else {
        Err(ProtocolErrors::CrcMismatch)
    }
}

fn fill_buffer_with_iterator(buffer_to_fill : &mut [u8], iterator: impl Iterator::<Item = u8>) -> Result<usize, ProtocolErrors> {
    let mut bytes_written = 0;

    for byte in iterator {
        let slot = buffer_to_fill.get_mut(bytes_written).ok_or(ProtocolErrors::OutputBufferTooSmall)?;

        *slot = byte;
        bytes_written += 1;
    }

    Ok(bytes_written)
}
