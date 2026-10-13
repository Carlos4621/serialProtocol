use std::panic::{AssertUnwindSafe, catch_unwind};

use serial_protocol::serial_protocol::{
    ProtocolErrors, decode, encode, min_decode_output_buffer_size, min_encode_output_buffer_size,
};

fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0; min_encode_output_buffer_size(payload.len())];
    let written = match encode(payload, &mut frame) {
        Ok(written) => written,
        Err(_) => panic!("a buffer sized by min_encode_output_buffer_size must be sufficient"),
    };
    frame.truncate(written);
    frame
}

fn assert_error_is(result: Result<usize, ProtocolErrors>, expected: fn(&ProtocolErrors) -> bool) {
    match result {
        Err(error) if expected(&error) => {}
        Err(_) => panic!("the operation returned a different protocol error"),
        Ok(written) => panic!("the operation unexpectedly succeeded and wrote {written} bytes"),
    }
}

#[test]
fn encode_matches_crc_and_cobsr_reference_vector() {
    // CRC-16/IBM-3740("123456789") = 0x29B1. COBS/R moves the trailing
    // 0xB1 into the first code position, and the protocol appends 0x00.
    let frame = encode_frame(b"123456789");

    assert_eq!(frame, b"\xB1123456789\x29\x00");
}

#[test]
fn non_empty_payloads_round_trip_with_an_exact_decode_buffer() {
    let payloads: [&[u8]; 5] = [
        b"a",
        b"hello serial protocol",
        b"contains\0embedded\0zeroes",
        &[0, 0, 0, 0],
        &[0xFF; 300],
    ];

    for payload in payloads {
        let frame = encode_frame(payload);
        // The decoded representation contains the payload plus its two CRC bytes.
        let mut decoded = vec![0xA5; payload.len() + 2];

        let written = match decode(&frame, &mut decoded) {
            Ok(written) => written,
            Err(_) => panic!("a frame produced by encode must be accepted by decode"),
        };

        assert_eq!(written, payload.len());
        assert_eq!(&decoded[..written], payload);
    }
}

#[test]
fn empty_payload_round_trips() {
    let frame = encode_frame(b"");
    let mut decoded = [0xA5; 2];

    let written = match decode(&frame, &mut decoded) {
        Ok(written) => written,
        Err(_) => panic!("decode rejected the empty frame produced by encode"),
    };

    assert_eq!(written, 0);
}

#[test]
fn decode_buffer_size_helper_is_sufficient_for_a_valid_frame() {
    let payload = b"payload";
    let frame = encode_frame(payload);
    let mut decoded = vec![0xA5; min_decode_output_buffer_size(frame.len())];

    let written = match decode(&frame, &mut decoded) {
        Ok(written) => written,
        Err(_) => panic!("the decode buffer size advertised by the API was not usable"),
    };

    assert_eq!(written, payload.len());
    assert_eq!(&decoded[..written], payload);
}

#[test]
fn encode_buffer_size_helper_is_sufficient_at_a_cobs_block_boundary() {
    // This payload has 253 non-zero bytes and CRC 0x1000. Consequently the
    // encoded input ends in a full 254-byte non-zero run followed by zero,
    // which exercises the COBS/R boundary where an extra code byte is needed.
    let mut payload = vec![1; 253];
    payload[252] = 145;
    let advertised_size = min_encode_output_buffer_size(payload.len());
    let mut frame = vec![0; advertised_size];

    let outcome = catch_unwind(AssertUnwindSafe(|| encode(&payload, &mut frame)));

    match outcome {
        Ok(Ok(written)) => assert!(
            written <= advertised_size,
            "encode wrote more than the size advertised by its helper"
        ),
        Ok(Err(_)) => panic!("encode rejected the size advertised by its own helper"),
        Err(_) => panic!("encode panicked with the size advertised by its own helper"),
    }
}

#[test]
fn encode_rejects_a_buffer_smaller_than_its_declared_minimum() {
    let payload = b"payload";
    let mut output = vec![0; min_encode_output_buffer_size(payload.len()) - 1];

    assert_error_is(encode(payload, &mut output), |error| {
        matches!(error, ProtocolErrors::OutputBufferTooSmall)
    });
}

#[test]
fn decode_rejects_a_buffer_too_small_for_payload_and_crc() {
    let payload = b"payload";
    let frame = encode_frame(payload);
    let mut output = vec![0; payload.len() + 1];

    assert_error_is(decode(&frame, &mut output), |error| {
        matches!(error, ProtocolErrors::OutputBufferTooSmall)
    });
}

#[test]
fn decode_rejects_crc_corruption() {
    let mut frame = encode_frame(b"123456789");
    frame[1] ^= 0x01;
    let mut output = [0; 11];

    assert_error_is(decode(&frame, &mut output), |error| {
        matches!(error, ProtocolErrors::CrcMismatch)
    });
}

#[test]
fn decode_rejects_a_non_zero_frame_delimiter() {
    let payload = b"payload";
    let mut frame = encode_frame(payload);
    *frame.last_mut().unwrap() = 0x7E;
    let mut output = vec![0; payload.len() + 2];

    assert_error_is(decode(&frame, &mut output), |error| {
        matches!(error, ProtocolErrors::InputWithoutDelimiter)
    });
}

#[test]
fn decode_rejects_truncated_input() {
    for frame in [b"".as_slice(), b"\x01".as_slice(), b"\x01\x00".as_slice()] {
        let mut output = [0; 2];
        assert_error_is(decode(frame, &mut output), |error| {
            matches!(error, ProtocolErrors::InputBufferTooSmall)
        });
    }
}

#[test]
fn decode_rejects_malformed_cobs_without_panicking() {
    // A zero inside the encoded region is forbidden by COBS/R. The output is
    // prefilled with the valid CRC of an empty payload to ensure input
    // validation cannot accidentally depend on existing output bytes.
    let malformed_frame = [0x00, 0x01, 0x01, 0x00];
    let mut output = [0xFF, 0xFF];

    let outcome = catch_unwind(AssertUnwindSafe(|| decode(&malformed_frame, &mut output)));

    match outcome {
        Ok(result) => assert_error_is(result, |error| {
            matches!(error, ProtocolErrors::InputBufferTooSmall)
        }),
        Err(_) => panic!("decode panicked for malformed input"),
    }
}
