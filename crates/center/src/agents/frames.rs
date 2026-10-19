use super::AgentError;
const MAX_FRAME_BYTES: usize = 4096;
#[derive(Default)]
pub(super) struct Frames {
    line: Vec<u8>,
    data: Vec<u8>,
    event: Option<String>,
    frame_bytes: usize,
}
pub(super) enum Frame {
    Snapshot(Box<hmail_cleaner_protocol::JobSnapshot>),
    Resync,
}
impl Frames {
    pub(super) fn push(&mut self, byte: u8) -> Result<Option<Frame>, AgentError> {
        self.frame_bytes += 1;
        if self.frame_bytes > MAX_FRAME_BYTES {
            return Err(AgentError::Protocol);
        }
        if byte != b'\n' {
            self.line.push(byte);
            return Ok(None);
        }
        let bytes = std::mem::take(&mut self.line);
        let line = std::str::from_utf8(&bytes)
            .map_err(|_| AgentError::Protocol)?
            .trim_end_matches('\r');
        if line.is_empty() {
            self.frame_bytes = 0;
            let kind = self.event.take();
            let data = std::mem::take(&mut self.data);
            return match kind.as_deref() {
                Some("snapshot") => Ok(Some(Frame::Snapshot(Box::new(
                    serde_json::from_slice(&data).map_err(|_| AgentError::Protocol)?,
                )))),
                Some("resync") => Ok(Some(Frame::Resync)),
                None => Ok(None),
                _ => Err(AgentError::Protocol),
            };
        }
        if let Some(event) = line.strip_prefix("event:") {
            self.event = Some(event.trim_start_matches(' ').into());
        } else if let Some(data) = line.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push(b'\n');
            }
            self.data
                .extend_from_slice(data.trim_start_matches(' ').as_bytes());
        } else if !line.starts_with(':') {
            return Err(AgentError::Protocol);
        }
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_crlf_heartbeat_resync_and_oversized_frame() {
        let mut frames = Frames::default();
        for byte in b":heartbeat\r\n\r\n" {
            assert!(frames.push(*byte).unwrap().is_none());
        }
        let mut result = None;
        for byte in b"event: resync\r\ndata: {}\r\n\r\n" {
            result = frames.push(*byte).unwrap().or(result);
        }
        assert!(matches!(result, Some(Frame::Resync)));
        for _ in 0..MAX_FRAME_BYTES {
            assert!(frames.push(b'x').unwrap().is_none());
        }
        assert!(frames.push(b'\n').is_err());
    }
}
