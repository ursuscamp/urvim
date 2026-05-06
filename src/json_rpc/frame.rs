use std::fmt;

const HEADER_SEPARATOR: &[u8] = b"\r\n\r\n";
const CONTENT_LENGTH_HEADER: &str = "content-length";
const CONTENT_TYPE_HEADER: &str = "content-type";
const DEFAULT_CONTENT_TYPE: &str = "application/vscode-jsonrpc; charset=utf-8";

/// A decoded content-length frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedFrame<'a> {
    /// The framed payload bytes.
    pub payload: &'a [u8],
    /// The number of bytes consumed from the input slice.
    pub consumed: usize,
}

/// Content-length framing for JSON-RPC payloads.
pub struct ContentLengthFrame;

impl ContentLengthFrame {
    /// Encodes a JSON payload into an LSP-style content-length frame.
    pub fn encode(payload: &[u8]) -> Vec<u8> {
        let mut framed = format!("Content-Length: {}\r\n", payload.len()).into_bytes();
        framed.extend_from_slice(b"Content-Type: ");
        framed.extend_from_slice(DEFAULT_CONTENT_TYPE.as_bytes());
        framed.extend_from_slice(HEADER_SEPARATOR);
        framed.extend_from_slice(payload);
        framed
    }

    /// Decodes the first content-length frame from the provided bytes.
    pub fn decode(input: &[u8]) -> Result<DecodedFrame<'_>, FrameError> {
        let header_end = find_header_end(input).ok_or(FrameError::MissingHeaderSeparator)?;
        let header_bytes = &input[..header_end];
        let payload_start = header_end + HEADER_SEPARATOR.len();
        let header_text =
            std::str::from_utf8(header_bytes).map_err(|_| FrameError::InvalidHeaderEncoding)?;

        let mut content_length = None;
        let mut content_type = None;

        for line in header_text.split("\r\n") {
            if line.is_empty() {
                continue;
            }

            let Some((name, value)) = line.split_once(':') else {
                return Err(FrameError::MalformedHeader(line.to_string()));
            };

            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();

            match name.as_str() {
                CONTENT_LENGTH_HEADER => {
                    let parsed = value
                        .parse::<usize>()
                        .map_err(|_| FrameError::InvalidContentLength(value.to_string()))?;
                    content_length = Some(parsed);
                }
                CONTENT_TYPE_HEADER => content_type = Some(value.to_string()),
                _ => {}
            }
        }

        let content_length = content_length.ok_or(FrameError::MissingContentLength)?;
        if !content_type_supported(content_type.as_deref()) {
            return Err(FrameError::UnsupportedContentType(
                content_type.unwrap_or_else(|| DEFAULT_CONTENT_TYPE.to_string()),
            ));
        }

        let payload_end = payload_start
            .checked_add(content_length)
            .ok_or(FrameError::TruncatedPayload)?;
        let payload = input
            .get(payload_start..payload_end)
            .ok_or(FrameError::TruncatedPayload)?;

        Ok(DecodedFrame {
            payload,
            consumed: payload_end,
        })
    }
}

fn find_header_end(input: &[u8]) -> Option<usize> {
    input
        .windows(HEADER_SEPARATOR.len())
        .position(|window| window == HEADER_SEPARATOR)
}

fn content_type_supported(content_type: Option<&str>) -> bool {
    match content_type {
        None => true,
        Some(value) => {
            let lowered = value.to_ascii_lowercase();
            lowered.contains("charset=utf-8") || lowered == DEFAULT_CONTENT_TYPE
        }
    }
}

/// Errors that can occur while parsing or serializing framed JSON-RPC payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// The frame does not contain the required header separator.
    MissingHeaderSeparator,
    /// The frame is missing a `Content-Length` header.
    MissingContentLength,
    /// The `Content-Length` header could not be parsed.
    InvalidContentLength(String),
    /// The header block is not valid UTF-8 / ASCII.
    InvalidHeaderEncoding,
    /// A header line did not contain a name/value separator.
    MalformedHeader(String),
    /// The content type requested an unsupported charset.
    UnsupportedContentType(String),
    /// The payload ended before the declared content length.
    TruncatedPayload,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeaderSeparator => write!(f, "missing header separator"),
            Self::MissingContentLength => write!(f, "missing Content-Length header"),
            Self::InvalidContentLength(value) => write!(f, "invalid Content-Length value: {value}"),
            Self::InvalidHeaderEncoding => write!(f, "header block is not valid ascii/utf-8"),
            Self::MalformedHeader(line) => write!(f, "malformed header line: {line}"),
            Self::UnsupportedContentType(value) => write!(f, "unsupported content type: {value}"),
            Self::TruncatedPayload => write!(f, "frame payload ended before declared length"),
        }
    }
}

impl std::error::Error for FrameError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_adds_content_length_and_payload() {
        let framed = ContentLengthFrame::encode(br#"{"jsonrpc":"2.0"}"#);
        let decoded = ContentLengthFrame::decode(&framed).expect("frame");
        assert_eq!(decoded.payload, br#"{"jsonrpc":"2.0"}"#);
        assert_eq!(decoded.consumed, framed.len());
    }

    #[test]
    fn decode_rejects_missing_length() {
        let bytes = b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}";
        let error = ContentLengthFrame::decode(bytes).expect_err("missing length");
        assert_eq!(error, FrameError::MissingContentLength);
    }

    #[test]
    fn decode_rejects_truncated_payload() {
        let bytes = b"Content-Length: 10\r\n\r\n{}";
        let error = ContentLengthFrame::decode(bytes).expect_err("truncated");
        assert_eq!(error, FrameError::TruncatedPayload);
    }
}
