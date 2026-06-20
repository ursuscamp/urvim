//! Length-prefixed MessagePack codec for process-backed plugins.

use std::io::{Read, Write};

use super::{PluginLoadError, PluginMessage};

/// Largest accepted plugin protocol frame.
pub const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

/// Encodes a plugin message as a length-prefixed MessagePack frame.
pub fn encode_frame(message: &PluginMessage) -> Result<Vec<u8>, PluginLoadError> {
    let payload = rmp_serde::to_vec_named(message)
        .map_err(|error| PluginLoadError::protocol(error.to_string()))?;
    if payload.len() > MAX_FRAME_LEN {
        return Err(PluginLoadError::protocol(format!(
            "plugin frame payload exceeds {MAX_FRAME_LEN} bytes"
        )));
    }

    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

/// Decodes a length-prefixed MessagePack frame into a plugin message.
pub fn decode_frame(frame: &[u8]) -> Result<PluginMessage, PluginLoadError> {
    if frame.len() < 4 {
        return Err(PluginLoadError::protocol(
            "plugin frame is missing length prefix",
        ));
    }
    let len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
    if len > MAX_FRAME_LEN {
        return Err(PluginLoadError::protocol(format!(
            "plugin frame length {len} exceeds {MAX_FRAME_LEN} bytes"
        )));
    }
    if frame.len() != 4 + len {
        return Err(PluginLoadError::protocol(format!(
            "plugin frame length prefix {len} does not match {} payload bytes",
            frame.len().saturating_sub(4)
        )));
    }

    rmp_serde::from_slice(&frame[4..]).map_err(|error| PluginLoadError::protocol(error.to_string()))
}

/// Writes a single plugin message frame.
pub fn write_frame(
    writer: &mut impl Write,
    message: &PluginMessage,
) -> Result<(), PluginLoadError> {
    let frame = encode_frame(message)?;
    writer
        .write_all(&frame)
        .map_err(|error| PluginLoadError::runtime(error.to_string()))
}

/// Reads a single plugin message frame.
pub fn read_frame(reader: &mut impl Read) -> Result<PluginMessage, PluginLoadError> {
    let mut header = [0; 4];
    reader
        .read_exact(&mut header)
        .map_err(|error| PluginLoadError::runtime(error.to_string()))?;
    let len = u32::from_be_bytes(header) as usize;
    if len > MAX_FRAME_LEN {
        return Err(PluginLoadError::protocol(format!(
            "plugin frame length {len} exceeds {MAX_FRAME_LEN} bytes"
        )));
    }

    let mut frame = Vec::with_capacity(4 + len);
    frame.extend_from_slice(&header);
    frame.resize(4 + len, 0);
    reader
        .read_exact(&mut frame[4..])
        .map_err(|error| PluginLoadError::runtime(error.to_string()))?;
    decode_frame(&frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PluginNotification, PluginRequest, PluginResponse};
    use serde_json::json;

    #[test]
    fn codec_round_trips_request_response_and_notification() {
        let messages = [
            PluginMessage::Request(PluginRequest::new(1, "editor/initialize", json!({"v": 1}))),
            PluginMessage::Response(PluginResponse::success(1, json!({"ok": true}))),
            PluginMessage::Notification(PluginNotification::new(
                "editor/notify",
                json!({"message": "hello"}),
            )),
        ];

        for message in messages {
            let frame = encode_frame(&message).expect("message should encode");
            let decoded = decode_frame(&frame).expect("message should decode");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn codec_rejects_invalid_frame_length() {
        let frame = [0, 0, 0, 8, 1, 2, 3];

        let error = decode_frame(&frame).expect_err("length mismatch should fail");

        assert!(error.to_string().contains("does not match"));
    }
}
