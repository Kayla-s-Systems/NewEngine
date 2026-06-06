use crate::error::{Result, TextureContainerError};
use crate::manifest::{TextureDictionaryManifest, TextureEntryMeta, TextureMipMeta};
use crate::{
    COLOR_SPACE_LINEAR, COLOR_SPACE_SRGB, PIXEL_FORMAT_BC1_RGBA_SRGB,
    PIXEL_FORMAT_BC1_RGBA_UNORM, PIXEL_FORMAT_BC2_RGBA_SRGB,
    PIXEL_FORMAT_BC2_RGBA_UNORM, PIXEL_FORMAT_BC3_RGBA_SRGB,
    PIXEL_FORMAT_BC3_RGBA_UNORM, PIXEL_FORMAT_BC5_RG_UNORM,
    PIXEL_FORMAT_BC6H_SF16, PIXEL_FORMAT_BC6H_UF16,
    PIXEL_FORMAT_BC7_RGBA_SRGB, PIXEL_FORMAT_BC7_RGBA_UNORM,
    PIXEL_FORMAT_RGBA8_SRGB, PIXEL_FORMAT_RGBA8_UNORM, VERSION_V2,
};

pub const DIRECTORY_MAGIC: [u8; 4] = *b"NTDX";
pub const DIRECTORY_VERSION: u16 = 1;
pub const DIRECTORY_HEADER_LEN: usize = 40;
pub const ENTRY_RECORD_LEN: usize = 64;
pub const MIP_RECORD_LEN: usize = 32;

const FORMAT_RGBA8_UNORM: u16 = 1;
const FORMAT_RGBA8_SRGB: u16 = 2;
const FORMAT_BC1_RGBA_UNORM: u16 = 101;
const FORMAT_BC1_RGBA_SRGB: u16 = 102;
const FORMAT_BC3_RGBA_UNORM: u16 = 103;
const FORMAT_BC3_RGBA_SRGB: u16 = 104;
const FORMAT_BC5_RG_UNORM: u16 = 105;
const FORMAT_BC7_RGBA_UNORM: u16 = 106;
const FORMAT_BC7_RGBA_SRGB: u16 = 107;
const FORMAT_BC2_RGBA_UNORM: u16 = 108;
const FORMAT_BC2_RGBA_SRGB: u16 = 109;
const FORMAT_BC6H_UF16: u16 = 110;
const FORMAT_BC6H_SF16: u16 = 111;
const COLOR_LINEAR: u16 = 1;
const COLOR_SRGB: u16 = 2;

#[derive(Debug, Clone, Copy)]
pub struct BinaryDirectoryStats {
    pub entry_count: u32,
    pub mip_count: u32,
}

pub fn encode(manifest: &TextureDictionaryManifest) -> Result<Vec<u8>> {
    let entry_count = u32::try_from(manifest.entries.len())
        .map_err(|_| TextureContainerError::DirectoryTooLarge("entry_count"))?;
    let mip_count_usize: usize = manifest.entries.iter().map(|entry| entry.mips.len()).sum();
    let mip_count = u32::try_from(mip_count_usize)
        .map_err(|_| TextureContainerError::DirectoryTooLarge("mip_count"))?;

    let entries_offset = DIRECTORY_HEADER_LEN as u32;
    let mips_offset = entries_offset
        .checked_add(entry_count.checked_mul(ENTRY_RECORD_LEN as u32).ok_or(TextureContainerError::DirectoryTooLarge("entries"))?)
        .ok_or(TextureContainerError::DirectoryTooLarge("mips_offset"))?;
    let names_offset = mips_offset
        .checked_add(mip_count.checked_mul(MIP_RECORD_LEN as u32).ok_or(TextureContainerError::DirectoryTooLarge("mips"))?)
        .ok_or(TextureContainerError::DirectoryTooLarge("names_offset"))?;

    let mut names = Vec::<u8>::new();
    let mut out = vec![0u8; names_offset as usize];
    let mut mip_cursor = 0u32;

    write_header(&mut out[..DIRECTORY_HEADER_LEN], entry_count, mip_count, entries_offset, mips_offset, names_offset, 0);

    for (entry_index, entry) in manifest.entries.iter().enumerate() {
        let name_offset = u32::try_from(names.len()).map_err(|_| TextureContainerError::DirectoryTooLarge("name_offset"))?;
        let name_bytes = entry.name.as_bytes();
        let name_len = u16::try_from(name_bytes.len()).map_err(|_| TextureContainerError::NameTooLong(entry.name.clone()))?;
        names.extend_from_slice(name_bytes);

        let entry_record_offset = entries_offset as usize + entry_index * ENTRY_RECORD_LEN;
        write_entry_record(
            &mut out[entry_record_offset..entry_record_offset + ENTRY_RECORD_LEN],
            entry,
            name_offset,
            name_len,
            mip_cursor,
        )?;

        for mip in &entry.mips {
            let mip_index = mip_cursor as usize;
            let mip_record_offset = mips_offset as usize + mip_index * MIP_RECORD_LEN;
            write_mip_record(&mut out[mip_record_offset..mip_record_offset + MIP_RECORD_LEN], mip);
            mip_cursor = mip_cursor
                .checked_add(1)
                .ok_or(TextureContainerError::DirectoryTooLarge("mip_cursor"))?;
        }
    }

    let names_len = u32::try_from(names.len()).map_err(|_| TextureContainerError::DirectoryTooLarge("names_len"))?;
    write_u32(&mut out, 32, names_len);
    out.extend_from_slice(&names);
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<TextureDictionaryManifest> {
    let header = read_header(bytes)?;
    let entries_start = header.entries_offset as usize;
    let mips_start = header.mips_offset as usize;
    let names_start = header.names_offset as usize;
    let names_len = header.names_len as usize;

    checked_range(bytes.len(), entries_start, header.entry_count as usize * ENTRY_RECORD_LEN, "directory.entries")?;
    checked_range(bytes.len(), mips_start, header.mip_count as usize * MIP_RECORD_LEN, "directory.mips")?;
    checked_range(bytes.len(), names_start, names_len, "directory.names")?;

    let names = &bytes[names_start..names_start + names_len];
    let mut entries = Vec::with_capacity(header.entry_count as usize);

    for i in 0..header.entry_count as usize {
        let offset = entries_start + i * ENTRY_RECORD_LEN;
        let record = &bytes[offset..offset + ENTRY_RECORD_LEN];
        let name_hash = read_u64(record, 0);
        let byte_offset = read_u64(record, 8);
        let byte_len = read_u64(record, 16);
        let name_offset = read_u32(record, 24) as usize;
        let name_len = read_u16(record, 28) as usize;
        let format_id = read_u16(record, 30);
        let width = read_u32(record, 32);
        let height = read_u32(record, 36);
        let first_mip = read_u32(record, 40) as usize;
        let mip_count = read_u32(record, 44) as usize;
        let color_space_id = read_u16(record, 48);

        checked_range(names.len(), name_offset, name_len, "entry.name")?;
        checked_range(header.mip_count as usize, first_mip, mip_count, "entry.mips")?;
        let name = std::str::from_utf8(&names[name_offset..name_offset + name_len])
            .map_err(|_| TextureContainerError::InvalidDirectory("entry name is not utf-8"))?
            .to_owned();
        let format = format_from_id(format_id, &name)?.to_owned();
        let color_space = color_space_from_id(color_space_id, &name)?.to_owned();

        let mut mips = Vec::with_capacity(mip_count);
        for mip_index in first_mip..first_mip + mip_count {
            let mip_offset = mips_start + mip_index * MIP_RECORD_LEN;
            let mip_record = &bytes[mip_offset..mip_offset + MIP_RECORD_LEN];
            mips.push(TextureMipMeta {
                byte_offset: read_u64(mip_record, 0),
                byte_len: read_u64(mip_record, 8),
                width: read_u32(mip_record, 16),
                height: read_u32(mip_record, 20),
                level: read_u16(mip_record, 24) as u32,
            });
        }

        entries.push(TextureEntryMeta {
            name,
            name_hash,
            width,
            height,
            format,
            color_space,
            byte_offset,
            byte_len,
            mip_count: mip_count as u32,
            mips,
        });
    }

    Ok(TextureDictionaryManifest {
        version: VERSION_V2,
        default_format: PIXEL_FORMAT_RGBA8_UNORM.to_owned(),
        entries,
    })
}

pub fn sniff(bytes: &[u8]) -> bool {
    bytes.len() >= DIRECTORY_HEADER_LEN && &bytes[0..4] == DIRECTORY_MAGIC.as_slice()
}

#[derive(Debug, Clone, Copy)]
struct DirectoryHeader {
    entry_count: u32,
    mip_count: u32,
    entries_offset: u32,
    mips_offset: u32,
    names_offset: u32,
    names_len: u32,
}

fn read_header(bytes: &[u8]) -> Result<DirectoryHeader> {
    if bytes.len() < DIRECTORY_HEADER_LEN {
        return Err(TextureContainerError::InvalidDirectory("short binary directory header"));
    }
    if &bytes[0..4] != DIRECTORY_MAGIC.as_slice() {
        return Err(TextureContainerError::InvalidDirectory("bad binary directory magic"));
    }
    let version = read_u16(bytes, 4);
    if version != DIRECTORY_VERSION {
        return Err(TextureContainerError::InvalidDirectory("unsupported binary directory version"));
    }
    let entry_record_len = read_u16(bytes, 6);
    let mip_record_len = read_u16(bytes, 8);
    if entry_record_len as usize != ENTRY_RECORD_LEN || mip_record_len as usize != MIP_RECORD_LEN {
        return Err(TextureContainerError::InvalidDirectory("unsupported binary directory record sizes"));
    }
    Ok(DirectoryHeader {
        entry_count: read_u32(bytes, 12),
        mip_count: read_u32(bytes, 16),
        entries_offset: read_u32(bytes, 20),
        mips_offset: read_u32(bytes, 24),
        names_offset: read_u32(bytes, 28),
        names_len: read_u32(bytes, 32),
    })
}

fn write_header(out: &mut [u8], entry_count: u32, mip_count: u32, entries_offset: u32, mips_offset: u32, names_offset: u32, names_len: u32) {
    out.fill(0);
    out[0..4].copy_from_slice(&DIRECTORY_MAGIC);
    write_u16(out, 4, DIRECTORY_VERSION);
    write_u16(out, 6, ENTRY_RECORD_LEN as u16);
    write_u16(out, 8, MIP_RECORD_LEN as u16);
    write_u16(out, 10, 0);
    write_u32(out, 12, entry_count);
    write_u32(out, 16, mip_count);
    write_u32(out, 20, entries_offset);
    write_u32(out, 24, mips_offset);
    write_u32(out, 28, names_offset);
    write_u32(out, 32, names_len);
}

fn write_entry_record(out: &mut [u8], entry: &TextureEntryMeta, name_offset: u32, name_len: u16, first_mip: u32) -> Result<()> {
    out.fill(0);
    write_u64(out, 0, entry.name_hash);
    write_u64(out, 8, entry.byte_offset);
    write_u64(out, 16, entry.byte_len);
    write_u32(out, 24, name_offset);
    write_u16(out, 28, name_len);
    write_u16(out, 30, format_to_id(&entry.format, &entry.name)?);
    write_u32(out, 32, entry.width);
    write_u32(out, 36, entry.height);
    write_u32(out, 40, first_mip);
    write_u32(out, 44, entry.mips.len() as u32);
    write_u16(out, 48, color_space_to_id(&entry.color_space, &entry.name)?);
    Ok(())
}

fn write_mip_record(out: &mut [u8], mip: &TextureMipMeta) {
    out.fill(0);
    write_u64(out, 0, mip.byte_offset);
    write_u64(out, 8, mip.byte_len);
    write_u32(out, 16, mip.width);
    write_u32(out, 20, mip.height);
    write_u16(out, 24, mip.level as u16);
}

fn format_to_id(format: &str, name: &str) -> Result<u16> {
    match format {
        PIXEL_FORMAT_RGBA8_UNORM => Ok(FORMAT_RGBA8_UNORM),
        PIXEL_FORMAT_RGBA8_SRGB => Ok(FORMAT_RGBA8_SRGB),
        PIXEL_FORMAT_BC1_RGBA_UNORM => Ok(FORMAT_BC1_RGBA_UNORM),
        PIXEL_FORMAT_BC1_RGBA_SRGB => Ok(FORMAT_BC1_RGBA_SRGB),
        PIXEL_FORMAT_BC2_RGBA_UNORM => Ok(FORMAT_BC2_RGBA_UNORM),
        PIXEL_FORMAT_BC2_RGBA_SRGB => Ok(FORMAT_BC2_RGBA_SRGB),
        PIXEL_FORMAT_BC3_RGBA_UNORM => Ok(FORMAT_BC3_RGBA_UNORM),
        PIXEL_FORMAT_BC3_RGBA_SRGB => Ok(FORMAT_BC3_RGBA_SRGB),
        PIXEL_FORMAT_BC5_RG_UNORM => Ok(FORMAT_BC5_RG_UNORM),
        PIXEL_FORMAT_BC6H_UF16 => Ok(FORMAT_BC6H_UF16),
        PIXEL_FORMAT_BC6H_SF16 => Ok(FORMAT_BC6H_SF16),
        PIXEL_FORMAT_BC7_RGBA_UNORM => Ok(FORMAT_BC7_RGBA_UNORM),
        PIXEL_FORMAT_BC7_RGBA_SRGB => Ok(FORMAT_BC7_RGBA_SRGB),
        other => Err(TextureContainerError::InvalidFormat { name: name.to_owned(), format: other.to_owned() }),
    }
}

fn format_from_id(id: u16, name: &str) -> Result<&'static str> {
    match id {
        FORMAT_RGBA8_UNORM => Ok(PIXEL_FORMAT_RGBA8_UNORM),
        FORMAT_RGBA8_SRGB => Ok(PIXEL_FORMAT_RGBA8_SRGB),
        FORMAT_BC1_RGBA_UNORM => Ok(PIXEL_FORMAT_BC1_RGBA_UNORM),
        FORMAT_BC1_RGBA_SRGB => Ok(PIXEL_FORMAT_BC1_RGBA_SRGB),
        FORMAT_BC2_RGBA_UNORM => Ok(PIXEL_FORMAT_BC2_RGBA_UNORM),
        FORMAT_BC2_RGBA_SRGB => Ok(PIXEL_FORMAT_BC2_RGBA_SRGB),
        FORMAT_BC3_RGBA_UNORM => Ok(PIXEL_FORMAT_BC3_RGBA_UNORM),
        FORMAT_BC3_RGBA_SRGB => Ok(PIXEL_FORMAT_BC3_RGBA_SRGB),
        FORMAT_BC5_RG_UNORM => Ok(PIXEL_FORMAT_BC5_RG_UNORM),
        FORMAT_BC6H_UF16 => Ok(PIXEL_FORMAT_BC6H_UF16),
        FORMAT_BC6H_SF16 => Ok(PIXEL_FORMAT_BC6H_SF16),
        FORMAT_BC7_RGBA_UNORM => Ok(PIXEL_FORMAT_BC7_RGBA_UNORM),
        FORMAT_BC7_RGBA_SRGB => Ok(PIXEL_FORMAT_BC7_RGBA_SRGB),
        _ => Err(TextureContainerError::InvalidFormat { name: name.to_owned(), format: format!("id:{id}") }),
    }
}

fn color_space_to_id(color_space: &str, name: &str) -> Result<u16> {
    match color_space {
        COLOR_SPACE_LINEAR => Ok(COLOR_LINEAR),
        COLOR_SPACE_SRGB => Ok(COLOR_SRGB),
        other => Err(TextureContainerError::InvalidColorSpace { name: name.to_owned(), color_space: other.to_owned() }),
    }
}

fn color_space_from_id(id: u16, name: &str) -> Result<&'static str> {
    match id {
        COLOR_LINEAR => Ok(COLOR_SPACE_LINEAR),
        COLOR_SRGB => Ok(COLOR_SPACE_SRGB),
        _ => Err(TextureContainerError::InvalidColorSpace { name: name.to_owned(), color_space: format!("id:{id}") }),
    }
}

fn checked_range(total: usize, offset: usize, len: usize, what: &'static str) -> Result<()> {
    let end = offset.checked_add(len).ok_or(TextureContainerError::InvalidRange {
        what,
        offset: offset as u64,
        len: len as u64,
        total,
    })?;
    if offset > total || end > total {
        return Err(TextureContainerError::InvalidRange {
            what,
            offset: offset as u64,
            len: len as u64,
            total,
        });
    }
    Ok(())
}

#[inline]
fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
}

#[inline]
fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3],
        bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7],
    ])
}

#[inline]
fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[inline]
fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[inline]
fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
