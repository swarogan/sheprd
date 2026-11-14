//! Framing: whole frames out of bytes however they arrive, and the parser
//! never panicking on arbitrary input.

use crate::frame::{Decoder, MAX_FRAME, encode};

#[test]
fn frames_come_out_whole_however_bytes_arrive() {
    let messages: Vec<Vec<u8>> = vec![b"hello".to_vec(), vec![], vec![9; 70_00], vec![1; 65_535]];
    let mut stream = Vec::new();
    for message in &messages {
        stream.extend(encode(message).unwrap());
    }
    for chunk in [1, 2, 3, 1_000, stream.len()] {
        let mut decoder = Decoder::default();
        let mut out = Vec::new();
        for piece in stream.chunks(chunk) {
            decoder.push(piece);
            while let Some(frame) = decoder.next_frame() {
                out.push(frame);
            }
        }
        assert_eq!(out, messages, "chunk {chunk}");
        assert_eq!(decoder.held(), 0);
    }
    assert!(encode(&vec![0; 65_536]).is_err());
}

#[test]
fn reading_what_the_decoder_wants_never_holds_more_than_a_frame() {
    let stream = [encode(&[1; 500]).unwrap(), encode(&[2; 10]).unwrap()].concat();
    let mut decoder = Decoder::default();
    let mut offset = 0;
    while offset < stream.len() {
        let take = decoder.wanted().min(stream.len() - offset);
        decoder.push(&stream[offset..offset + take]);
        offset += take;
        assert!(decoder.held() <= MAX_FRAME);
        while decoder.next_frame().is_some() {}
    }
}

#[test]
fn arbitrary_bytes_never_panic_the_decoder_or_a_record() {
    // A small, deterministic stand-in for fuzzing: a linear congruential
    // stream of bytes through the decoder and into `open`.
    let (_, mut listening) = super::session(&super::keys(1), &super::keys(2));
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut decoder = Decoder::default();
    for _ in 0..2_000 {
        let mut chunk = [0; 97];
        for byte in &mut chunk {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            *byte = (state >> 56) as u8;
        }
        decoder.push(&chunk);
        while let Some(frame) = decoder.next_frame() {
            let _ = listening.open(&frame);
        }
        if decoder.held() > 4 * MAX_FRAME {
            decoder = Decoder::default();
        }
    }
}
