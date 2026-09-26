//! RC-S380 (Sony NFC Port-100) access and FeliCa frame construction.
//!
//! # Why the frames are built by hand
//!
//! `felica` keeps its command serializer private (`felica_standard::command::serialize`
//! is a private module), and the high-level `FelicaStandard::authentication1`
//! takes `challenge_1a` as an argument but performs the *rest* of the session
//! locally with keys this process does not have. Neither fits a PoC whose whole
//! point is that the oracle holds the keys.
//!
//! So every frame below is assembled explicitly. That is also what makes the
//! transcript readable: each step prints the exact bytes in both directions.
//!
//! # Wire format
//!
//! A FeliCa command frame is a one-byte total length (counting itself),
//! followed by the command code, followed by the payload:
//!
//! ```text
//! +------+------+------------------------+
//! | LEN  | CMD  | payload ...            |
//! +------+------+------------------------+
//!    1      1        LEN-2
//! ```
//!
//! Every addressed response echoes the length, carries `CMD + 1` as its
//! response code, and repeats the card's 8-byte IDm at offset 2.

use anyhow::{bail, Context, Result};
use felica::felica_standard::{FelicaDriver, FelicaStandard, ServiceCode};
use felica::{open_reader, RemoteTarget, Reader, ReaderPreference};

/// Command codes, from `felica_standard::constants`. A response always uses
/// `command + 1`.
pub mod code {
    pub const REQUEST_SERVICE: u8 = 0x02;
    pub const REQUEST_SYSTEM_CODE: u8 = 0x0C;
    pub const REQUEST_BLOCK_INFORMATION: u8 = 0x0E;
    pub const AUTHENTICATION1: u8 = 0x10;
    pub const AUTHENTICATION2: u8 = 0x12;
    pub const REQUEST_CODE_LIST: u8 = 0x1A;
}

/// One 8-byte challenge block.
pub type Block = [u8; 8];

/// Everything a key-free `Request Service` reveals.
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// One key version per requested node, in request order.
    pub key_versions: Vec<u16>,
}

/// The result of `Request Code List`: the card's area/service map.
#[derive(Debug, Clone)]
pub struct CodeList {
    pub continue_flag: u8,
    /// `(area code, end service code)` pairs bounding each area.
    pub areas: Vec<(u16, u16)>,
    pub services: Vec<u16>,
}

/// A card that has been polled and is ready for addressed commands.
pub struct Card {
    reader: Reader,
    target: RemoteTarget,
    /// Bitrate string reported by polling, e.g. `"424F"`.
    pub bitrate: String,
    /// 8-byte Manufacture ID, read in cleartext during polling.
    pub idm: Block,
    /// 8-byte Manufacture Parameter, also cleartext.
    pub pmm: [u8; 8],
    /// Polling optional data (communication performance, when requested).
    pub polling_optional: Vec<u8>,
}

impl Card {
    /// Open a reader and poll until a card answers, or `wait` seconds elapse.
    ///
    /// `424F` is preferred over `212F`; `felica`'s `polling_multi` orders them
    /// that way itself, and a Suica in a gate or on a desktop reader will
    /// normally activate at 424 kbps.
    pub fn poll(
        preference: ReaderPreference,
        system_code: u16,
        request_code: u8,
        time_slots: u8,
        wait_secs: u64,
    ) -> Result<Self> {
        let mut reader = open_reader(preference)
            .context("opening the reader — check the USB cable and the udev rule")?;

        println!("Reader: {}", describe(&reader));
        println!("Polling for a card (system code {system_code:04X}) for up to {wait_secs}s...");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait_secs);

        let (bitrate, idm, pmm, optional) = loop {
            match poll_once(reader.driver_mut(), system_code, request_code, time_slots) {
                Ok(found) => break found,
                Err(err) => {
                    if std::time::Instant::now() >= deadline {
                        bail!(
                            "no card answered within {wait_secs}s (last attempt: {err})\n\
                             hint: tap the card flat on the reader and hold it there"
                        );
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            }
        };

        println!("Card found at {bitrate}.");
        let target = RemoteTarget::new(bitrate.clone())
            .with_context(|| format!("reconstructing a target for bitrate {bitrate}"))?;

        Ok(Self {
            reader,
            target,
            bitrate,
            idm,
            pmm,
            polling_optional: optional,
        })
    }

    /// Send one already-framed command and return the raw response frame.
    pub fn transceive(&mut self, frame: &[u8], timeout_ms: u16) -> Result<Vec<u8>> {
        self.reader
            .driver_mut()
            .transceive(&self.target, frame, Some(timeout_ms))
            .with_context(|| format!("transceive {}", hex::encode(frame)))
    }

    /// `Request Service` — asks the card for the key version of each node.
    ///
    /// This is key-free and reveals whether the oracle's provisioned key
    /// version (`0000`, the DES set) is the one the card actually uses.
    pub fn request_service(&mut self, nodes: &[u16], timeout_ms: u16) -> Result<NodeInfo> {
        let mut payload = vec![code::REQUEST_SERVICE];
        payload.extend_from_slice(&self.idm);
        payload.push(nodes.len() as u8);
        for node in nodes {
            payload.extend_from_slice(&node.to_le_bytes());
        }
        let frame = frame(payload)?;

        let rx = self.transceive(&frame, timeout_ms)?;
        expect_response_code(&rx, code::REQUEST_SERVICE + 1, "Request Service")?;
        if rx.len() < 11 {
            bail!("Request Service response too short: {} bytes", rx.len());
        }
        let count = rx[10] as usize;
        if rx.len() != 11 + count * 2 {
            bail!(
                "Request Service response length {} disagrees with node count {count}",
                rx.len()
            );
        }
        let key_versions = rx[11..11 + count * 2]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(NodeInfo { key_versions })
    }

    /// `Request Code List` — the card's own area/service map.
    ///
    /// Purely informational: the node path used for authentication comes from
    /// the oracle, not from here.
    pub fn request_code_list(
        &mut self,
        parent: u16,
        index: u16,
        timeout_ms: u16,
    ) -> Result<CodeList> {
        let mut payload = vec![code::REQUEST_CODE_LIST];
        payload.extend_from_slice(&self.idm);
        payload.extend_from_slice(&parent.to_le_bytes());
        payload.extend_from_slice(&index.to_le_bytes());
        let frame = frame(payload)?;

        let rx = self.transceive(&frame, timeout_ms)?;
        expect_response_code(&rx, code::REQUEST_CODE_LIST + 1, "Request Code List")?;
        if rx.len() < 12 {
            bail!("Request Code List response too short: {} bytes", rx.len());
        }
        let (sf1, sf2) = (rx[10], rx[11]);
        if sf1 != 0 {
            bail!("Request Code List rejected: SF1={sf1:02X} SF2={sf2:02X}");
        }
        if rx.len() < 15 {
            bail!("Request Code List success response too short: {} bytes", rx.len());
        }
        let continue_flag = rx[12];
        let area_count = rx[13] as usize;
        let mut off = 14;
        if rx.len() < off + area_count * 4 + 1 {
            bail!("Request Code List truncated inside the area table");
        }
        let mut areas = Vec::with_capacity(area_count);
        for chunk in rx[off..off + area_count * 4].chunks_exact(4) {
            areas.push((
                u16::from_le_bytes([chunk[0], chunk[1]]),
                u16::from_le_bytes([chunk[2], chunk[3]]),
            ));
        }
        off += area_count * 4;
        let service_count = rx[off] as usize;
        off += 1;
        if rx.len() != off + service_count * 2 {
            bail!(
                "Request Code List length {} disagrees with service count {service_count}",
                rx.len()
            );
        }
        let services = rx[off..off + service_count * 2]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(CodeList {
            continue_flag,
            areas,
            services,
        })
    }

    /// `Request Block Information` — how many 16-byte blocks each node holds.
    ///
    /// Key-free. Purely descriptive: the PoC reads no card data, it only
    /// proves possession of the card's key schedule.
    pub fn request_block_information(&mut self, nodes: &[u16], timeout_ms: u16) -> Result<Vec<u16>> {
        let mut payload = vec![code::REQUEST_BLOCK_INFORMATION];
        payload.extend_from_slice(&self.idm);
        payload.push(nodes.len() as u8);
        for node in nodes {
            payload.extend_from_slice(&node.to_le_bytes());
        }
        let frame = frame(payload)?;

        let rx = self.transceive(&frame, timeout_ms)?;
        expect_response_code(&rx, code::REQUEST_BLOCK_INFORMATION + 1, "Request Block Information")?;
        if rx.len() < 11 {
            bail!("Request Block Information response too short: {} bytes", rx.len());
        }
        let count = rx[10] as usize;
        if rx.len() != 11 + count * 2 {
            bail!(
                "Request Block Information length {} disagrees with node count {count}",
                rx.len()
            );
        }
        Ok(rx[11..11 + count * 2]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect())
    }

    /// `Request System Code` — every system code the card answers to.
    pub fn request_system_codes(&mut self, timeout_ms: u16) -> Result<Vec<u16>> {
        let mut payload = vec![code::REQUEST_SYSTEM_CODE];
        payload.extend_from_slice(&self.idm);
        let frame = frame(payload)?;

        let rx = self.transceive(&frame, timeout_ms)?;
        expect_response_code(&rx, code::REQUEST_SYSTEM_CODE + 1, "Request System Code")?;
        if rx.len() < 11 {
            bail!("Request System Code response too short: {} bytes", rx.len());
        }
        let count = rx[10] as usize;
        if rx.len() != 11 + count * 2 {
            bail!("Request System Code length disagrees with count {count}");
        }
        Ok(rx[11..11 + count * 2]
            .chunks_exact(2)
            // System codes are big-endian on the wire, unlike node codes.
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect())
    }

    /// `Authentication1` — the step the oracle participates in.
    ///
    /// We supply C1A (computed by the oracle from our R1) together with the node
    /// path. The card replies with **both** C1B and C2A, so a single exchange
    /// yields everything `settle` needs.
    ///
    /// Returns `(c1b, c2a)`.
    pub fn authentication1(
        &mut self,
        areas: &[u16],
        services: &[ServiceCode],
        c1a: &Block,
        timeout_ms: u16,
    ) -> Result<(Block, Block)> {
        let mut payload = vec![code::AUTHENTICATION1];
        payload.extend_from_slice(&self.idm);
        payload.push(areas.len() as u8);
        for area in areas {
            payload.extend_from_slice(&area.to_le_bytes());
        }
        payload.push(services.len() as u8);
        for service in services {
            payload.extend_from_slice(&service.raw().to_le_bytes());
        }
        payload.extend_from_slice(c1a);
        let frame = frame(payload)?;

        let rx = self.transceive(&frame, timeout_ms)?;
        expect_response_code(&rx, code::AUTHENTICATION1 + 1, "Authentication1")?;
        if rx.len() != 26 {
            bail!("Authentication1 response must be 26 bytes, got {}", rx.len());
        }
        // If the card answered at all, it echoed the IDm we addressed. A
        // mismatch means the card on the reader changed mid-session.
        let echoed: Block = rx[2..10].try_into().expect("length checked above");
        if echoed != self.idm {
            bail!(
                "Authentication1 response echoed IDm {} but we addressed {}",
                hex::encode_upper(echoed),
                hex::encode_upper(self.idm)
            );
        }
        let c1b: Block = rx[10..18].try_into().expect("8 bytes");
        let c2a: Block = rx[18..26].try_into().expect("8 bytes");
        Ok((c1b, c2a))
    }

    /// `Authentication2` — hand the card its C2B and collect the AUTH2 frame.
    ///
    /// The response is 34 bytes: a 2-byte MAC header (total length, opcode
    /// `13h`) followed by 32 bytes of CBC ciphertext under R2. Those 32 bytes
    /// are the `auth2` blob the oracle needs — they are what the IDi is hidden
    /// inside, and this process cannot decrypt them.
    pub fn authentication2(&mut self, c2b: &Block, timeout_ms: u16) -> Result<Vec<u8>> {
        let mut payload = vec![code::AUTHENTICATION2];
        payload.extend_from_slice(&self.idm);
        payload.extend_from_slice(c2b);
        let frame = frame(payload)?;

        let rx = self.transceive(&frame, timeout_ms)?;
        expect_response_code(&rx, code::AUTHENTICATION2 + 1, "Authentication2")?;
        if rx.len() != 34 {
            bail!("Authentication2 response must be 34 bytes, got {}", rx.len());
        }
        if rx[1] != 0x13 {
            bail!("Authentication2 response opcode is {:02X}, expected 13", rx[1]);
        }
        // 34 == 2 + 24 (TN‖TID‖IDi‖PMi) + 8 (MAC): the plaintext is already
        // block-aligned, so unlike the reader-to-card direction there is no
        // PKCS#7 padding here.
        Ok(rx[2..].to_vec())
    }
}

/// Prepend the one-byte total-length field.
fn frame(payload: Vec<u8>) -> Result<Vec<u8>> {
    let len = payload.len() + 1;
    if len > 0xFF {
        bail!("FeliCa packet would be {len} bytes; the length field caps it at 255");
    }
    let mut out = Vec::with_capacity(len);
    out.push(len as u8);
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Reject anything that is not the response we asked for.
///
/// A wrong response code here almost always means the card refused the command,
/// so its status flags are worth surfacing rather than swallowing.
fn expect_response_code(rx: &[u8], want: u8, what: &str) -> Result<()> {
    if rx.len() < 2 {
        bail!("{what}: card returned {}-byte frame, too short for a response code", rx.len());
    }
    if rx[0] as usize != rx.len() {
        bail!(
            "{what}: length byte {:02X} disagrees with the {} bytes received",
            rx[0],
            rx.len()
        );
    }
    if rx[1] != want {
        let detail = if rx.len() >= 12 {
            format!(" (status flags {:02X} {:02X})", rx[10], rx[11])
        } else {
            String::new()
        };
        bail!(
            "{what}: card answered {:02X}, expected {:02X}{detail}\n\
             hint: for Authentication1 this usually means C1A was wrong, i.e. the \
             oracle's gsk/usk do not match this card, or the node path is wrong",
            rx[1],
            want
        );
    }
    Ok(())
}

type PolledCard = (String, Block, [u8; 8], Vec<u8>);

/// One polling attempt. Returns `(bitrate, IDm, PMm, optional data)`.
fn poll_once(
    driver: &mut dyn FelicaDriver,
    system_code: u16,
    request_code: u8,
    time_slots: u8,
) -> Result<PolledCard> {
    let (felica, result) = FelicaStandard::polling_multi(
        driver,
        &["212F", "424F"],
        system_code,
        request_code,
        time_slots,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let bitrate = felica.bitrate().to_string();
    let idm: Block = felica
        .idm()
        .try_into()
        .map_err(|_| anyhow::anyhow!("IDm must be 8 bytes"))?;
    let pmm: [u8; 8] = felica
        .pmm()
        .try_into()
        .map_err(|_| anyhow::anyhow!("PMm must be 8 bytes"))?;
    let optional = result.optional.clone();
    Ok((bitrate, idm, pmm, optional))
}

fn describe(reader: &Reader) -> String {
    let vendor = reader.vendor_name().unwrap_or("?");
    let product = reader.product_name().unwrap_or("?");
    let chipset = reader.chipset_name();
    format!("{vendor} {product} [{chipset}]")
}
