//! iNES ROM image parsing using nom combinators.
//!
//! An iNES file has a 16-byte header followed by PRG-ROM (code)
//! and optional CHR-ROM (graphics) banks. The header encodes the
//! mapper number, which determines how the cartridge's address
//! space is wired.
//!
//! ## iNES header layout (bytes 0–15)
//!
//! | Offset | Content                               |
//! |--------|---------------------------------------|
//! | 0–3    | Magic: `NES\x1A`                      |
//! | 4      | PRG-ROM size in 16 KB units           |
//! | 5      | CHR-ROM size in 8 KB units            |
//! | 6      | Flags 6 (mapper low, mirroring, etc.) |
//! | 7      | Flags 7 (mapper high, format)         |
//! | 8–15   | Extended flags / padding              |

#![allow(dead_code)]

use nom::Parser;
use nom::bytes::complete::{tag, take};
use nom::combinator::cond;
use nom::number::complete::le_u8;

/// Expected magic bytes at the start of an iNES file.
const INES_MAGIC: &[u8; 4] = b"NES\x1a";

/// Size of one PRG-ROM bank.
const PRG_BANK_SIZE: usize = 16_384;

/// Size of one CHR-ROM bank.
const CHR_BANK_SIZE: usize = 8_192;

/// Nametable mirroring mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mirroring {
    /// Horizontal arrangement (vertical mirroring).
    Horizontal,
    /// Vertical arrangement (horizontal mirroring).
    Vertical,
    /// Four-screen VRAM (cartridge-provided).
    FourScreen,
}

/// Parsed iNES header fields (internal to the parser).
struct Header {
    prg_banks: u8,
    chr_banks: u8,
    flags6: u8,
    flags7: u8,
}

impl Header {
    /// Mapper ID from flags 6 and 7.
    fn mapper_id(&self) -> u8 {
        (self.flags7 & 0xF0) | (self.flags6 >> 4)
    }

    /// Nametable mirroring mode from flags 6.
    fn mirroring(&self) -> Mirroring {
        if self.flags6 & 0x08 != 0 {
            Mirroring::FourScreen
        } else if self.flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        }
    }

    /// Whether a 512-byte trainer is present before PRG-ROM.
    fn has_trainer(&self) -> bool {
        self.flags6 & 0x04 != 0
    }
}

/// Parsed iNES cartridge image.
pub(crate) struct Cartridge {
    /// Program ROM.
    prg_rom: Vec<u8>,
    /// Character ROM (tile/sprite data). Empty if CHR-RAM is used.
    chr_rom: Vec<u8>,
    /// Mapper number (iNES mapper ID).
    mapper_id: u8,
    /// Nametable mirroring mode.
    mirroring: Mirroring,
}

impl Cartridge {
    /// Parses an iNES-format ROM image from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is not a valid iNES image
    /// (bad magic, truncated, or malformed header).
    pub(crate) fn from_ines(data: &[u8]) -> anyhow::Result<Self> {
        let (_, cart) = parse_ines(data).map_err(|e| anyhow::anyhow!("iNES parse error: {e}"))?;
        tracing::info!(
            mapper_id = cart.mapper_id,
            prg_size = cart.prg_rom.len(),
            chr_size = cart.chr_rom.len(),
            mirroring = ?cart.mirroring,
            "parsed iNES ROM",
        );
        Ok(cart)
    }

    /// Returns the mapper ID.
    pub(super) fn mapper_id(&self) -> u8 {
        self.mapper_id
    }

    /// Returns the PRG-ROM data.
    pub(super) fn prg_rom(&self) -> &[u8] {
        &self.prg_rom
    }

    /// Returns the CHR-ROM data.
    pub(super) fn chr_rom(&self) -> &[u8] {
        &self.chr_rom
    }

    /// Returns the nametable mirroring mode.
    pub(super) fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

// ── nom parsers ──────────────────────────────────────────────────

/// Parses the 16-byte iNES header into a [`Header`].
fn parse_header(input: &[u8]) -> nom::IResult<&[u8], Header> {
    (
        tag(INES_MAGIC.as_slice()),
        le_u8,
        le_u8,
        le_u8,
        le_u8,
        take(8_usize), // bytes 8–15: padding / extended flags
    )
        .map(
            |(_, prg_banks, chr_banks, flags6, flags7, _padding)| Header {
                prg_banks,
                chr_banks,
                flags6,
                flags7,
            },
        )
        .parse(input)
}

/// Parses a complete iNES ROM image into a [`Cartridge`].
fn parse_ines(input: &[u8]) -> nom::IResult<&[u8], Cartridge> {
    let (input, header) = parse_header(input)?;

    // Skip the optional 512-byte trainer.
    let (input, _trainer) = cond(header.has_trainer(), take(512_usize)).parse(input)?;

    let prg_size = usize::from(header.prg_banks) * PRG_BANK_SIZE;
    let chr_size = usize::from(header.chr_banks) * CHR_BANK_SIZE;

    let (input, prg_rom) = take(prg_size).parse(input)?;
    let (input, chr_rom) = take(chr_size).parse(input)?;

    let cart = Cartridge {
        prg_rom: prg_rom.to_vec(),
        chr_rom: chr_rom.to_vec(),
        mapper_id: header.mapper_id(),
        mirroring: header.mirroring(),
    };

    Ok((input, cart))
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Builds a minimal valid iNES image with the given PRG and CHR data.
    fn make_ines(prg: &[u8], chr: &[u8], flags6: u8, flags7: u8) -> Vec<u8> {
        let prg_banks = prg.len() / PRG_BANK_SIZE;
        let chr_banks = chr.len() / CHR_BANK_SIZE;
        let mut data = Vec::new();
        data.extend_from_slice(INES_MAGIC);
        data.push(prg_banks as u8);
        data.push(chr_banks as u8);
        data.push(flags6);
        data.push(flags7);
        data.extend_from_slice(&[0u8; 8]); // padding
        data.extend_from_slice(prg);
        data.extend_from_slice(chr);
        data
    }

    #[test]
    fn parse_minimal_rom() {
        let prg = vec![0xEA; PRG_BANK_SIZE]; // one bank of NOPs
        let rom = make_ines(&prg, &[], 0x00, 0x00);

        let cart = Cartridge::from_ines(&rom).expect("should parse valid minimal ROM");

        assert_eq!(cart.prg_rom().len(), PRG_BANK_SIZE);
        assert!(cart.chr_rom().is_empty());
        assert_eq!(cart.mapper_id(), 0);
        assert_eq!(cart.mirroring(), Mirroring::Horizontal);
    }

    #[test]
    fn parse_mapper_and_mirroring() {
        let prg = vec![0x00; PRG_BANK_SIZE];
        // flags6 = 0x11: vertical mirroring, mapper low nibble = 1
        // flags7 = 0x20: mapper high nibble = 2
        // mapper = 0x20 | 0x01 = 0x21 = 33
        let rom = make_ines(&prg, &[], 0x11, 0x20);

        let cart = Cartridge::from_ines(&rom).expect("should parse mapper/mirroring");

        assert_eq!(cart.mapper_id(), 0x21);
        assert_eq!(cart.mirroring(), Mirroring::Vertical);
    }

    #[test]
    fn parse_four_screen_mirroring() {
        let prg = vec![0x00; PRG_BANK_SIZE];
        let rom = make_ines(&prg, &[], 0x08, 0x00);

        let cart = Cartridge::from_ines(&rom).expect("should parse four-screen mirroring");

        assert_eq!(cart.mirroring(), Mirroring::FourScreen);
    }

    #[test]
    fn parse_with_chr_rom() {
        let prg = vec![0x00; PRG_BANK_SIZE];
        let chr = vec![0xFF; CHR_BANK_SIZE * 2]; // 2 CHR banks
        let rom = make_ines(&prg, &chr, 0x00, 0x00);

        let cart = Cartridge::from_ines(&rom).expect("should parse CHR-ROM");

        assert_eq!(cart.chr_rom().len(), CHR_BANK_SIZE * 2);
    }

    #[test]
    fn parse_with_trainer() {
        let prg = vec![0x00; PRG_BANK_SIZE];
        let mut rom = make_ines(&prg, &[], 0x04, 0x00);
        // Insert 512-byte trainer after header, before PRG.
        let trainer = vec![0xAB; 512];
        let _: Vec<u8> = rom.splice(16..16, trainer).collect();

        let cart = Cartridge::from_ines(&rom).expect("should parse ROM with trainer");

        assert_eq!(cart.prg_rom().len(), PRG_BANK_SIZE);
    }

    #[test]
    fn reject_bad_magic() {
        let data = b"NOT_NES_DATA_AT_ALL!";
        let result = Cartridge::from_ines(data);
        assert!(result.is_err());
    }

    #[test]
    fn reject_truncated_data() {
        // Valid header but no PRG data.
        let mut data = Vec::new();
        data.extend_from_slice(INES_MAGIC);
        data.push(1); // 1 PRG bank = 16KB expected
        data.push(0);
        data.extend_from_slice(&[0u8; 10]); // rest of header

        let result = Cartridge::from_ines(&data);
        assert!(result.is_err());
    }
}
