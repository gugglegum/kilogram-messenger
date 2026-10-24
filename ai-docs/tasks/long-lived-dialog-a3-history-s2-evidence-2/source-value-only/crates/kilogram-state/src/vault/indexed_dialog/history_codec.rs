//! Only the unsigned/bstr/array CBOR subset used by the local history DTO.
use super::{Error, HistoryPage, Result};
use serde::Serialize;

const CBOR_CAP: usize = 256 * 1024;
const JSON_CAP: usize = 1024 * 1024;

fn head(out: &mut Vec<u8>, major: u8, value: u64) {
    match value {
        0..=23 => out.push((major << 5) | value as u8),
        24..=255 => out.extend([(major << 5) | 24, value as u8]),
        256..=65535 => {
            out.push((major << 5) | 25);
            out.extend((value as u16).to_be_bytes());
        }
        65536..=0xffff_ffff => {
            out.push((major << 5) | 26);
            out.extend((value as u32).to_be_bytes());
        }
        _ => {
            out.push((major << 5) | 27);
            out.extend(value.to_be_bytes());
        }
    }
}
fn bytes(out: &mut Vec<u8>, value: &[u8]) {
    head(out, 2, value.len() as u64);
    out.extend(value);
}
fn hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(2 * value.len());
    for byte in value {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 15)] as char);
    }
    out
}
#[derive(Serialize)]
struct JsonRow<'a> {
    id: String,
    role: u8,
    ordinal: u64,
    sequence: u64,
    request: String,
    text: &'a str,
}
#[derive(Serialize)]
struct JsonPage<'a> {
    version: u8,
    view: String,
    #[serde(rename = "F")]
    frontiers: [u64; 2],
    finished: bool,
    cursor: String,
    items: Vec<JsonRow<'a>>,
}
impl HistoryPage {
    fn check_encoding_input(&self) -> Result<()> {
        if self.items.len() > 32
            || self
                .items
                .iter()
                .any(|row| row.role > 1 || row.text.len() > 4096)
        {
            return Err(Error::Bound("history response fields"));
        }
        Ok(())
    }
    pub fn encode_cbor(&self) -> Result<Vec<u8>> {
        self.check_encoding_input()?;
        let mut out =
            Vec::with_capacity(335 + self.items.iter().map(|r| r.text.len() + 74).sum::<usize>());
        head(&mut out, 4, 7);
        head(&mut out, 0, 2);
        bytes(&mut out, &self.view);
        for n in self.frontiers {
            head(&mut out, 0, n);
        }
        head(&mut out, 0, u64::from(self.finished));
        bytes(&mut out, self.cursor.as_bytes());
        head(&mut out, 4, self.items.len() as u64);
        for row in &self.items {
            head(&mut out, 4, 6);
            bytes(&mut out, &row.id);
            head(&mut out, 0, u64::from(row.role));
            head(&mut out, 0, row.ordinal);
            head(&mut out, 0, row.sequence);
            bytes(&mut out, &row.request);
            bytes(&mut out, row.text.as_bytes());
        }
        if out.len() > CBOR_CAP {
            return Err(Error::Bound("history CBOR response"));
        }
        Ok(out)
    }
    pub fn encode_json(&self) -> Result<Vec<u8>> {
        self.check_encoding_input()?;
        let page = JsonPage {
            version: 2,
            view: hex(&self.view),
            frontiers: self.frontiers,
            finished: self.finished,
            cursor: self.cursor.to_base64(),
            items: self
                .items
                .iter()
                .map(|row| JsonRow {
                    id: hex(&row.id),
                    role: row.role,
                    ordinal: row.ordinal,
                    sequence: row.sequence,
                    request: hex(&row.request),
                    text: &row.text,
                })
                .collect(),
        };
        let out = serde_json::to_vec(&page).map_err(|_| Error::Bound("history JSON encoding"))?;
        if out.len() > JSON_CAP {
            return Err(Error::Bound("history JSON response"));
        }
        Ok(out)
    }
}
