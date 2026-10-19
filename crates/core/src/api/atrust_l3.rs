use sangfor_core::l3::{self, FrameDecoder, HandshakeParser};

const BUFFER_LIMIT: usize = 128 * 1024;

pub fn auth_tunnel_request(sid: String) -> anyhow::Result<Vec<u8>> {
    Ok(l3::auth_tunnel_request(&sid)?)
}

pub fn heartbeat_request() -> Vec<u8> {
    l3::heartbeat_request()
}

pub fn data_request(token: String, packet: Vec<u8>) -> anyhow::Result<Vec<u8>> {
    Ok(l3::data_request(&token, &packet)?)
}

#[derive(Debug, PartialEq, Eq)]
pub struct L3Frame {
    pub command: u8,
    pub status: u8,
    pub payload: Vec<u8>,
}

/// Rejects further input after a framing error until explicitly reset.
pub struct L3FrameDecoder {
    decoder: FrameDecoder,
    pending: usize,
    failed: bool,
}

impl Default for L3FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl L3FrameDecoder {
    pub fn new() -> Self {
        Self {
            decoder: FrameDecoder::new(),
            pending: 0,
            failed: false,
        }
    }

    pub fn push(&mut self, chunk: Vec<u8>) -> anyhow::Result<Vec<L3Frame>> {
        anyhow::ensure!(!self.failed, "L3 decoder must be reset after an error");
        if chunk.len() > BUFFER_LIMIT.saturating_sub(self.pending) {
            self.fail();
            anyhow::bail!("L3 frame buffer limit exceeded");
        }
        self.pending += chunk.len();
        let frames = match self.decoder.push(&chunk) {
            Ok(frames) => frames,
            Err(error) => {
                self.fail();
                return Err(error.into());
            }
        };
        Ok(frames
            .into_iter()
            .map(|frame| {
                self.pending -=
                    frame.payload.len() + if frame.command.has_status() { 5 } else { 4 };
                L3Frame {
                    command: frame.command as u8,
                    status: frame.status,
                    payload: frame.payload,
                }
            })
            .collect())
    }

    pub fn reset(&mut self) {
        self.decoder.reset();
        self.pending = 0;
        self.failed = false;
    }

    fn fail(&mut self) {
        self.decoder.reset();
        self.pending = 0;
        self.failed = true;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct L3Handshake {
    pub device_id: Option<String>,
    pub virtual_addresses: Vec<String>,
    pub remaining_bytes: Vec<u8>,
}

/// Consumes the VIP trailer with the auth response and preserves following frames.
pub struct L3HandshakeDecoder {
    parser: HandshakeParser,
    received: usize,
    finished: bool,
}

impl Default for L3HandshakeDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl L3HandshakeDecoder {
    pub fn new() -> Self {
        Self {
            parser: HandshakeParser::new(),
            received: 0,
            finished: false,
        }
    }

    pub fn push(&mut self, chunk: Vec<u8>) -> anyhow::Result<Option<L3Handshake>> {
        anyhow::ensure!(!self.finished, "L3 handshake decoder already finished");
        if chunk.len() > BUFFER_LIMIT.saturating_sub(self.received) {
            self.finished = true;
            anyhow::bail!("L3 handshake buffer limit exceeded");
        }
        self.received += chunk.len();
        match self.parser.push(&chunk) {
            Ok(Some((handshake, remaining_bytes))) => {
                self.finished = true;
                Ok(Some(L3Handshake {
                    device_id: handshake.device_id,
                    virtual_addresses: handshake.virtual_ip,
                    remaining_bytes,
                }))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                self.finished = true;
                Err(error.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handshake(address_type: u8, address: &[u8]) -> Vec<u8> {
        let payload = br#"{"code":0,"data":{"deviceId":"test-device"}}"#;
        let mut bytes = vec![0x05, 0xd0, 0x53, 0x00, 0x00, payload.len() as u8];
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(&[0x05, 0, 0, address_type]);
        bytes.extend_from_slice(address);
        bytes
    }

    #[test]
    fn opening_request_matches_pinned_sdk_golden() -> anyhow::Result<()> {
        let expected = [
            vec![5, 1, 0xd0, 0x53, 0, 0, 22],
            br#"{"sid":"REDACTED_SID"}"#.to_vec(),
            vec![5, 4, 0, 1, 0, 0, 0, 0, 0, 0],
        ]
        .concat();
        assert_eq!(auth_tunnel_request("REDACTED_SID".to_owned())?, expected);
        assert_eq!(heartbeat_request(), vec![5, 0x15, 0, 0]);
        Ok(())
    }

    #[test]
    fn data_request_checks_lengths_and_utf8_token_bytes() -> anyhow::Result<()> {
        let frame = data_request("令".to_owned(), vec![1, 2])?;
        assert_eq!(
            frame,
            [
                vec![5, 0x14, 3],
                "令".as_bytes().to_vec(),
                vec![0, 0, 1, 0, 2, 1, 2]
            ]
            .concat()
        );
        assert!(data_request("x".repeat(256), vec![]).is_err());
        assert!(data_request("token".to_owned(), vec![0; 65536]).is_err());
        Ok(())
    }

    #[test]
    fn frames_handle_fragmentation_coalescing_and_status() -> anyhow::Result<()> {
        let bytes = [5, 0x94, 0, 3, 1, 2, 3, 5, 0x93, 0x84, 0, 0, 5, 0x95, 0, 0];
        let mut decoder = L3FrameDecoder::new();
        let mut frames = vec![];
        for byte in bytes {
            frames.extend(decoder.push(vec![byte])?);
        }
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].payload, vec![1, 2, 3]);
        assert_eq!(frames[1].status, 0x84);
        assert_eq!(frames[2].command, 0x95);
        decoder.reset();
        assert_eq!(decoder.push(bytes.to_vec())?, frames);
        Ok(())
    }

    #[test]
    fn malformed_frame_stays_failed_until_reset() -> anyhow::Result<()> {
        let mut decoder = L3FrameDecoder::new();
        assert!(decoder.push(vec![5, 0]).is_err());
        assert!(decoder.push(heartbeat_request()).is_err());
        decoder.reset();
        assert_eq!(decoder.push(heartbeat_request())?.len(), 1);
        assert!(decoder.push(vec![0; BUFFER_LIMIT + 1]).is_err());
        Ok(())
    }

    #[test]
    fn handshake_preserves_following_frame_and_virtual_ip() -> anyhow::Result<()> {
        let opening = handshake(1, &[10, 95, 178, 77, 0, 0]);
        let after = [5, 0x95, 0, 0];
        for split in 0..=opening.len() {
            let mut decoder = L3HandshakeDecoder::new();
            let first = decoder.push(opening[..split].to_vec())?;
            let result = if let Some(result) = first {
                result
            } else {
                decoder
                    .push([opening[split..].to_vec(), after.to_vec()].concat())?
                    .ok_or_else(|| anyhow::anyhow!("handshake not completed"))?
            };
            assert_eq!(result.virtual_addresses, vec!["10.95.178.77"]);
            assert_eq!(result.device_id.as_deref(), Some("test-device"));
            assert_eq!(
                result.remaining_bytes,
                if split == opening.len() {
                    vec![]
                } else {
                    after.to_vec()
                }
            );
            assert!(decoder.push(vec![]).is_err());
        }
        Ok(())
    }

    #[test]
    fn handshake_rejects_status_unknown_address_and_oversized_input() {
        let mut denied = L3HandshakeDecoder::new();
        assert!(denied.push(vec![5, 0xd0, 0x53, 1, 0, 0]).is_err());
        assert!(denied.push(vec![]).is_err());
        assert!(L3HandshakeDecoder::new().push(handshake(9, &[])).is_err());
        assert!(L3HandshakeDecoder::new()
            .push(vec![0; BUFFER_LIMIT + 1])
            .is_err());
    }

    #[test]
    fn handshake_decodes_both_address_families() -> anyhow::Result<()> {
        let ipv6 = [
            0x20, 1, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
        ];
        let v6 = L3HandshakeDecoder::new()
            .push(handshake(4, &ipv6))?
            .ok_or_else(|| anyhow::anyhow!("missing IPv6 handshake"))?;
        assert_eq!(v6.virtual_addresses, vec!["2001:db8::1"]);
        let dual = [&[10, 0, 0, 42][..], &ipv6].concat();
        let dual = L3HandshakeDecoder::new()
            .push(handshake(5, &dual))?
            .ok_or_else(|| anyhow::anyhow!("missing dual stack handshake"))?;
        assert_eq!(dual.virtual_addresses, vec!["10.0.0.42", "2001:db8::1"]);
        Ok(())
    }
}
