use std::io;
use std::io::Result as IOResult;
use std::io::Read;
use std::error::Error;

use byteorder::{ReadBytesExt, BigEndian};

const FRAME_LEN_U16: u8 = 126;
const FRAME_LEN_U64: u8 = 127;

#[allow(dead_code)]
#[derive(Debug)]
pub struct WebSocketFrameHeader {  
    fin: bool,
    rsv1: bool,
    rsv2: bool,
    rsv3: bool,
    masked: bool,
    opcode: u8,
    payload_length: u8
}

#[derive(Debug)]
pub struct WebSocketFrame {
    header: WebSocketFrameHeader,
    pub payload: Vec<u8>
}

impl WebSocketFrame {
    pub fn read<R: Read>(input: &mut R) -> IOResult<WebSocketFrame> {
        let mut buf = [0u8; 2];
        try!(input.read(&mut buf));
        let header = Self::parse_header(buf);

        let len = try!(Self::read_length(header.payload_length, input));
        let mask_key = if header.masked {
            let mask = try!(Self::read_mask(input));
            Some(mask)
        } else {
            None
        };
        let mut payload = try!(Self::read_payload(len, input));

        if let Some(mask) = mask_key {
            Self::apply_mask(mask, &mut payload);
        }

        Ok(WebSocketFrame {
            header: header,
            payload: payload
        })
    }

    fn parse_header(buf: [u8; 2]) -> WebSocketFrameHeader {
        WebSocketFrameHeader {
            fin: buf[0] & 0x80 == 0x80,
            rsv1: buf[0] & 0x40 == 0x40,
            rsv2: buf[0] & 0x20 == 0x20,
            rsv3: buf[0] & 0x10 == 0x10,
            opcode: buf[0] & 0x0F,
            masked: buf[1] & 0x80 == 0x80,
            payload_length: buf[1] & 0x7F,
        }
    }

    fn apply_mask(mask: [u8; 4], bytes: &mut Vec<u8>) {
        for (idx, c) in bytes.iter_mut().enumerate() {
            *c = mask[idx % 4];
        }
    }

    fn read_mask<R: Read>(input: &mut R) -> IOResult<[u8; 4]> {
        let mut buf = [0; 4];
        try!(input.read(&mut buf));
        Ok(buf)
    }

    fn read_payload<R: Read>(payload_len: usize, input: &mut R) -> IOResult<Vec<u8>> {
        let mut payload: Vec<u8> = Vec::with_capacity(payload_len);
        unsafe { payload.set_len(payload_len) };
        try!(input.read(&mut payload));
        Ok(payload)
    }

    fn read_length<R: Read>(payload_len: u8, input: &mut R) -> Result<usize, io::Error> {
        return match payload_len {
            FRAME_LEN_U64 => input.read_u64::<BigEndian>().map(|v| v as usize).map_err(|e| io::Error::from(e)),
            FRAME_LEN_U16 => input.read_u16::<BigEndian>().map(|v| v as usize).map_err(|e| io::Error::from(e)),
            _ => Ok(payload_len as usize) // payload_len < 127
        }
    }
}
