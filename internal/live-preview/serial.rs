// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore jtag

//! The protocol that the LSP's live preview uses to show a design on a microcontroller,
//! over a serial port such as the USB-Serial-JTAG port of an ESP32-S3.
//! The LSP compiles the design and sends the serialized compilation unit;
//! the device only runs it. It builds without std.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Bumped when a message changes. Both ends must speak the same one.
pub const PROTOCOL_VERSION: u16 = 2;

/// The Slint version that both ends report in the handshake.
/// The host only sends a unit to a device of the same version.
pub const SLINT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The size of the pieces a unit is sent in.
pub const CHUNK_SIZE: usize = 16 * 1024;

/// Starts every frame, so that the device's log output can't pass for one.
const MAGIC: &[u8; 4] = b"SLP1";

/// What the desktop sends to the device.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum HostMessage {
    /// Asks the device for [`DeviceMessage::Ready`].
    Hello {
        protocol: u16,
    },
    /// Starts sending a blob for the device to keep under `key`, see [`blob_key()`]:
    /// `total_len` bytes follow in [`HostMessage::StoreChunk`]s.
    /// A `resource` is an embedded resource of a unit, which the device keeps until a
    /// [`HostMessage::Show`] that doesn't name it. Any other blob is a unit.
    StoreBegin {
        key: u64,
        total_len: u32,
        resource: bool,
    },
    StoreChunk {
        key: u64,
        offset: u32,
        data: Vec<u8>,
    },
    /// Show `component` of the unit stored as `unit`, with the embedded resources stored as
    /// `resources`, in order. The device then keeps no other unit, and no other resource.
    Show {
        id: u32,
        component: String,
        unit: u64,
        resources: Vec<u64>,
    },
    /// Show the idle screen.
    Clear,
    /// Restart the device.
    Reset,
}

/// What the device sends to the desktop.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DeviceMessage {
    /// Sent at startup, and in answer to [`HostMessage::Hello`].
    Ready {
        protocol: u16,
        /// The [`SLINT_VERSION`] and unit format the device's interpreter reads, see
        /// `i_slint_compiler::llr::serialize::FORMAT_ID`.
        slint_version: String,
        format: u64,
        board: String,
        width: u32,
        height: u32,
        scale_factor: f32,
        heap_free: u32,
    },
    /// The unit `id` is on screen.
    Loaded {
        id: u32,
        heap_free: u32,
    },
    Error {
        id: Option<u32>,
        message: String,
    },
}

/// `message`, as a frame to write to the port.
pub fn encode_frame<T: Serialize>(message: &T) -> Vec<u8> {
    let mut payload =
        postcard::to_extend(message, Vec::from(&MAGIC[..])).expect("messages always serialize");
    let crc = crc32(&payload);
    payload.extend(crc.to_le_bytes());
    let mut frame = Vec::with_capacity(cobs::max_encoding_length(payload.len()) + 2);
    frame.push(0);
    frame.extend(cobs::encode_vec(&payload));
    frame.push(0);
    frame
}

/// What [`FrameDecoder`] finds in the stream.
#[derive(Debug, PartialEq)]
pub enum Received<T> {
    Message(T),
    /// Bytes outside of a frame, such as log output; a line at a time where possible.
    Text(Vec<u8>),
}

/// Splits the bytes read from the port into messages and text.
///
/// Text never contains a zero byte, which delimits frames. A segment that doesn't decode
/// as a frame is reported as text, and its terminating zero taken as the start of a frame,
/// so the decoder finds the frames again after joining the stream in the middle of one.
pub struct FrameDecoder<T> {
    buffer: Vec<u8>,
    /// The decoded frame, kept to reuse its allocation.
    payload: Vec<u8>,
    in_frame: bool,
    max_frame_len: usize,
    _message: core::marker::PhantomData<T>,
}

impl<T: DeserializeOwned> FrameDecoder<T> {
    /// A decoder that drops frames longer than `max_frame_len` encoded bytes.
    pub fn new(max_frame_len: usize) -> Self {
        Self {
            buffer: Vec::new(),
            payload: Vec::new(),
            in_frame: false,
            max_frame_len,
            _message: core::marker::PhantomData,
        }
    }

    pub fn push(&mut self, mut bytes: &[u8], mut on_received: impl FnMut(Received<T>)) {
        while !bytes.is_empty() {
            let delimiter = bytes.iter().position(|&byte| byte == 0);
            let (segment, rest) = match delimiter {
                Some(pos) => (&bytes[..pos], &bytes[pos + 1..]),
                None => (bytes, &[][..]),
            };
            bytes = rest;
            if self.in_frame {
                if self.buffer.len() + segment.len() > self.max_frame_len {
                    self.buffer.clear();
                    self.in_frame = false;
                    // Too long for a frame: the rest of it is text, up to the next delimiter
                    self.push_text(segment, &mut on_received);
                } else {
                    self.buffer.extend_from_slice(segment);
                }
            } else {
                self.push_text(segment, &mut on_received);
            }
            if delimiter.is_some() {
                self.end_segment(&mut on_received);
            }
        }
    }

    /// Text between frames is reported a line at a time.
    fn push_text(&mut self, mut text: &[u8], on_received: &mut impl FnMut(Received<T>)) {
        while let Some(pos) = text.iter().position(|&byte| byte == b'\n') {
            self.buffer.extend_from_slice(&text[..=pos]);
            on_received(Received::Text(core::mem::take(&mut self.buffer)));
            text = &text[pos + 1..];
        }
        if self.buffer.len() + text.len() > self.max_frame_len {
            self.buffer.clear();
        }
        self.buffer.extend_from_slice(text);
    }

    fn end_segment(&mut self, on_received: &mut impl FnMut(Received<T>)) {
        if !self.in_frame {
            if !self.buffer.is_empty() {
                on_received(Received::Text(core::mem::take(&mut self.buffer)));
            }
            self.in_frame = true;
        } else if self.buffer.is_empty() {
            // Two delimiters in a row: still waiting for the frame
        } else if let Some(message) = decode_frame(&self.buffer, &mut self.payload) {
            self.buffer.clear();
            on_received(Received::Message(message));
            self.in_frame = false;
        } else {
            on_received(Received::Text(core::mem::take(&mut self.buffer)));
        }
    }
}

fn decode_frame<T: DeserializeOwned>(encoded: &[u8], payload: &mut Vec<u8>) -> Option<T> {
    payload.resize(encoded.len(), 0);
    let len = cobs::decode(encoded, payload).ok()?;
    let (body, crc) = payload[..len].split_at_checked(len.checked_sub(4)?)?;
    if crc32(body).to_le_bytes() != crc {
        return None;
    }
    postcard::from_bytes(body.strip_prefix(&MAGIC[..])?).ok()
}

/// CRC-32 (IEEE), as zlib computes it.
fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut crc = i as u32;
            let mut bit = 0;
            while bit < 8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
                bit += 1;
            }
            table[i] = crc;
            i += 1;
        }
        table
    };
    let mut crc = !0u32;
    for &byte in data {
        crc = TABLE[((crc ^ u32::from(byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

/// The key that the host stores `data` under: FNV-1a, so that equal blobs get equal keys.
pub fn blob_key(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The frames that store `data` on the device under `key`, see [`HostMessage::StoreBegin`].
pub fn store_frames(key: u64, data: &[u8], resource: bool) -> impl Iterator<Item = Vec<u8>> + '_ {
    let begin =
        encode_frame(&HostMessage::StoreBegin { key, total_len: data.len() as u32, resource });
    core::iter::once(begin).chain(data.chunks(CHUNK_SIZE).enumerate().map(move |(i, data)| {
        encode_frame(&HostMessage::StoreChunk {
            key,
            offset: (i * CHUNK_SIZE) as u32,
            data: data.into(),
        })
    }))
}

/// What the device has to act on, see [`DeviceSession::push`].
#[derive(Debug, PartialEq)]
pub enum DeviceEvent {
    /// Answer with [`DeviceMessage::Ready`].
    Hello,
    /// An embedded resource arrived, for the device to keep under `key`.
    Resource {
        key: u64,
        data: Vec<u8>,
    },
    /// Show the unit that [`DeviceSession::blob()`] returns, see [`HostMessage::Show`].
    Show {
        id: u32,
        component: String,
        unit: u64,
        resources: Vec<u64>,
    },
    Clear,
    Reset,
    /// Tell the host with [`DeviceMessage::Error`].
    Error(String),
}

struct PendingStore {
    key: u64,
    total_len: usize,
    resource: bool,
    data: Vec<u8>,
}

/// The device's end of the protocol: turns the bytes read from the port into events,
/// and keeps the units the host stored.
pub struct DeviceSession {
    decoder: FrameDecoder<HostMessage>,
    pending: Option<PendingStore>,
    blobs: BTreeMap<u64, Vec<u8>>,
    max_blob_len: usize,
}

impl DeviceSession {
    /// A session that refuses blobs bigger than `max_blob_len` bytes.
    pub fn new(max_blob_len: usize) -> Self {
        Self {
            decoder: FrameDecoder::new(cobs::max_encoding_length(CHUNK_SIZE + 64)),
            pending: None,
            blobs: BTreeMap::new(),
            max_blob_len,
        }
    }

    /// The blob stored under `key`, if it arrived whole.
    pub fn blob(&self, key: u64) -> Option<&[u8]> {
        self.blobs.get(&key).map(Vec::as_slice)
    }

    pub fn push(&mut self, bytes: &[u8], mut on_event: impl FnMut(DeviceEvent)) {
        let Self { decoder, pending, blobs, max_blob_len } = self;
        decoder.push(bytes, |received| {
            let Received::Message(message) = received else { return };
            let error = |message: String| Some(DeviceEvent::Error(message));
            let event = match message {
                HostMessage::Hello { protocol } if protocol == PROTOCOL_VERSION => {
                    Some(DeviceEvent::Hello)
                }
                HostMessage::Hello { protocol } => error(alloc::format!(
                    "The device speaks protocol {PROTOCOL_VERSION}, not {protocol}: flash the firmware built with this Slint version"
                )),
                HostMessage::StoreBegin { key, total_len, resource } => {
                    let total_len = total_len as usize;
                    *pending = None;
                    let mut data = Vec::new();
                    if total_len > *max_blob_len {
                        error(alloc::format!(
                            "The design needs {total_len} bytes, but the device only has room for {max_blob_len}"
                        ))
                    } else if data.try_reserve_exact(total_len).is_err() {
                        error(alloc::format!("Out of memory receiving {total_len} bytes"))
                    } else {
                        *pending = Some(PendingStore { key, total_len, resource, data });
                        None
                    }
                }
                HostMessage::StoreChunk { key, offset, data } => match pending.as_mut() {
                    Some(store) if store.key == key && store.data.len() == offset as usize => {
                        store.data.extend_from_slice(&data);
                        if store.data.len() < store.total_len {
                            None
                        } else {
                            let store = pending.take().unwrap();
                            if store.data.len() != store.total_len {
                                error("Received more data than announced".into())
                            } else if store.resource {
                                Some(DeviceEvent::Resource { key: store.key, data: store.data })
                            } else {
                                blobs.insert(store.key, store.data);
                                None
                            }
                        }
                    }
                    // A chunk of a store that was superseded or failed
                    Some(store) if store.key != key => None,
                    None => None,
                    Some(_) => {
                        *pending = None;
                        error("Parts of the design went missing".into())
                    }
                },
                HostMessage::Show { id, component, unit, resources } => {
                    blobs.retain(|key, _| *key == unit);
                    Some(DeviceEvent::Show { id, component, unit, resources })
                }
                HostMessage::Clear => Some(DeviceEvent::Clear),
                HostMessage::Reset => Some(DeviceEvent::Reset),
            };
            if let Some(event) = event {
                on_event(event);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn decode_all<T: DeserializeOwned>(bytes: &[u8]) -> Vec<Received<T>> {
        let mut decoder = FrameDecoder::new(1 << 20);
        let mut out = Vec::new();
        decoder.push(bytes, |r| out.push(r));
        out
    }

    #[test]
    fn crc32_matches_zlib() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn frames_round_trip_between_log_lines() {
        let message = DeviceMessage::Loaded { id: 3, heap_free: 1234 };
        let mut stream = b"booting\nstill ".to_vec();
        stream.extend(encode_frame(&message));
        stream.extend(b"booting\n");
        stream.extend(encode_frame(&message));
        assert_eq!(
            decode_all::<DeviceMessage>(&stream),
            vec![
                Received::Text(b"booting\n".to_vec()),
                Received::Text(b"still ".to_vec()),
                Received::Message(message.clone()),
                Received::Text(b"booting\n".to_vec()),
                Received::Message(message),
            ]
        );
    }

    #[test]
    fn a_damaged_frame_is_not_a_message() {
        let mut frame = encode_frame(&HostMessage::Clear);
        let middle = frame.len() / 2;
        frame[middle] ^= 0x40;
        assert!(
            decode_all::<HostMessage>(&frame).iter().all(|r| !matches!(r, Received::Message(_)))
        );
    }

    #[test]
    fn joining_mid_frame_resynchronizes() {
        let frame = encode_frame(&HostMessage::Reset);
        // The tail of a frame, then a whole one.
        let mut stream = frame[3..].to_vec();
        stream.extend(encode_frame(&HostMessage::Clear));
        let messages: Vec<_> = decode_all::<HostMessage>(&stream)
            .into_iter()
            .filter_map(|r| match r {
                Received::Message(m) => Some(m),
                Received::Text(_) => None,
            })
            .collect();
        assert_eq!(messages, vec![HostMessage::Clear]);
    }

    fn show(id: u32, unit: u64, resources: &[u64]) -> Vec<u8> {
        encode_frame(&HostMessage::Show {
            id,
            component: "Main".into(),
            unit,
            resources: resources.into(),
        })
    }

    #[test]
    fn a_blob_arrives_in_chunks() {
        let unit: Vec<u8> = (0..(CHUNK_SIZE * 2 + 17)).map(|i| (i % 251) as u8).collect();
        let key = blob_key(&unit);
        let mut session = DeviceSession::new(1 << 20);
        let mut events = Vec::new();
        // Feed the stream a few bytes at a time, as the port delivers it
        let mut stream: Vec<u8> = store_frames(key, &unit, false).flatten().collect();
        stream.extend(show(7, key, &[]));
        for piece in stream.chunks(61) {
            session.push(piece, |e| events.push(e));
        }
        assert_eq!(
            events,
            vec![DeviceEvent::Show {
                id: 7,
                component: "Main".into(),
                unit: key,
                resources: vec![]
            }]
        );
        assert_eq!(session.blob(key), Some(unit.as_slice()));
    }

    #[test]
    fn a_show_keeps_only_its_unit() {
        let mut session = DeviceSession::new(1 << 20);
        for unit in [&b"shown"[..], b"dropped"] {
            for frame in store_frames(blob_key(unit), unit, false) {
                session.push(&frame, |e| panic!("{e:?}"));
            }
        }
        let [shown, dropped] = [&b"shown"[..], b"dropped"].map(blob_key);
        session.push(&show(1, shown, &[]), |_| {});
        assert_eq!(session.blob(shown), Some(&b"shown"[..]));
        assert_eq!(session.blob(dropped), None);
    }

    #[test]
    fn a_resource_goes_to_the_device() {
        let mut session = DeviceSession::new(1 << 20);
        let mut events = Vec::new();
        for frame in store_frames(3, b"image", true) {
            session.push(&frame, |e| events.push(e));
        }
        assert_eq!(events, vec![DeviceEvent::Resource { key: 3, data: b"image".to_vec() }]);
        assert_eq!(session.blob(3), None);
    }

    #[test]
    fn a_missing_chunk_is_reported() {
        let unit = vec![1u8; CHUNK_SIZE * 2];
        let frames: Vec<_> = store_frames(1, &unit, false).collect();
        let mut session = DeviceSession::new(1 << 20);
        let mut events = Vec::new();
        session.push(&frames[0], |e| events.push(e));
        session.push(&frames[2], |e| events.push(e));
        assert!(matches!(events.as_slice(), [DeviceEvent::Error(_)]));
        assert_eq!(session.blob(1), None);
    }

    #[test]
    fn a_blob_too_big_is_refused() {
        let unit = vec![1u8; 100];
        let mut session = DeviceSession::new(10);
        let mut events = Vec::new();
        for frame in store_frames(2, &unit, false) {
            session.push(&frame, |e| events.push(e));
        }
        assert!(matches!(events.as_slice(), [DeviceEvent::Error(_)]));
    }

    #[test]
    fn a_newer_protocol_is_refused() {
        let mut session = DeviceSession::new(10);
        let mut events = Vec::new();
        session.push(&encode_frame(&HostMessage::Hello { protocol: PROTOCOL_VERSION + 1 }), |e| {
            events.push(e)
        });
        assert!(matches!(events.as_slice(), [DeviceEvent::Error(_)]));
    }
}
