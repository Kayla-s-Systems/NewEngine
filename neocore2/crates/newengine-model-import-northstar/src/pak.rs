use std::collections::BTreeMap;

const TLOU2_PC_MAGIC: u32 = 2681;
const TLOU2_RES_PADDING: usize = 48;
const TLOU2_LOGIN_SIGNATURE: u32 = 74_565;

#[derive(Clone, Copy, Debug)]
struct PakPage {
    offset: usize,
    size: usize,
}

#[derive(Clone, Debug)]
pub struct PakResource {
    pub kind: String,
    pub name: String,
    pub absolute_offset: usize,
    pub page_start: usize,
}

#[derive(Clone, Debug)]
pub struct PakFile {
    bytes: Vec<u8>,
    pages: Vec<PakPage>,
    pointer_targets: BTreeMap<usize, usize>,
    resources: Vec<PakResource>,
    tlou2_pc: bool,
}

impl PakFile {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        if read_u32(&bytes, 0)? != TLOU2_PC_MAGIC {
            return Err(format!(
                "North Star importer supports TLOU2 PC magic={} only, got {}",
                TLOU2_PC_MAGIC,
                read_u32(&bytes, 0)?
            ));
        }
        let login_page = read_u32(&bytes, 8)? as usize;
        let login_offset = read_u32(&bytes, 12)? as usize;
        let page_count = read_u32(&bytes, 16)? as usize;
        let page_table = read_u32(&bytes, 20)? as usize;
        let fixup_table = read_u32(&bytes, 28)? as usize;
        if page_count == 0 || page_count > 4096 {
            return Err(format!("invalid North Star page count {page_count}"));
        }
        let mut pages = Vec::with_capacity(page_count);
        for index in 0..page_count {
            let at = page_table
                .checked_add(index * 12)
                .ok_or("page table range overflow")?;
            let offset = read_u32(&bytes, at)? as usize;
            let size = read_u32(&bytes, at + 4)? as usize;
            let end = offset.checked_add(size).ok_or_else(|| {
                format!("page range overflow index={index} offset=0x{offset:x} size=0x{size:x}")
            })?;
            if end > bytes.len() {
                return Err(format!(
                    "page outside package index={index} offset=0x{offset:x} size=0x{size:x} package_size=0x{:x}",
                    bytes.len()
                ));
            }
            pages.push(PakPage { offset, size });
        }
        let login_page_desc = pages
            .get(login_page)
            .ok_or_else(|| format!("login page outside table index={login_page}"))?;
        if login_offset
            .checked_add(36)
            .is_none_or(|end| end > login_page_desc.size)
        {
            return Err(format!(
                "login resource outside page index={login_page} relative=0x{login_offset:x} page_size=0x{:x}",
                login_page_desc.size
            ));
        }
        let login_abs = login_page_desc
            .offset
            .checked_add(login_offset)
            .ok_or("login resource offset overflow")?;
        let tlou2_pc = read_u32(&bytes, login_abs + 32)? == TLOU2_LOGIN_SIGNATURE;
        if !tlou2_pc {
            return Err("package is not a TLOU2 PC resource layout".to_owned());
        }

        let fixup_data = read_u32(&bytes, fixup_table + 4)? as usize;
        let fixup_count = read_u32(&bytes, fixup_table + 8)? as usize;
        if fixup_count > 2_000_000 {
            return Err(format!(
                "pointer fixup count is unreasonable count={fixup_count}"
            ));
        }
        let mut pointer_targets = BTreeMap::new();
        for index in 0..fixup_count {
            let at = fixup_data
                .checked_add(index * 8)
                .ok_or("pointer fixup table overflow")?;
            let source_page = read_u16(&bytes, at)? as usize;
            let target_page = read_u16(&bytes, at + 2)? as usize;
            let relative_field = read_u32(&bytes, at + 4)? as usize;
            let source = pages
                .get(source_page)
                .ok_or_else(|| format!("pointer source page outside table {source_page}"))?;
            if target_page >= pages.len() {
                return Err(format!("pointer target page outside table {target_page}"));
            }
            if relative_field
                .checked_add(8)
                .is_none_or(|end| end > source.size)
            {
                return Err(format!(
                    "pointer field outside source page page={source_page} relative=0x{relative_field:x} page_size=0x{:x}",
                    source.size
                ));
            }
            let field = source
                .offset
                .checked_add(relative_field)
                .ok_or("pointer field address overflow")?;
            pointer_targets.insert(field, target_page);
        }

        let mut resources = Vec::new();
        for page in &pages {
            // TLOU2 ResPage header: page size at +12, entry count at +18, entries at +20.
            if page.size < 20 {
                continue;
            }
            let header_entries = read_u16(&bytes, page.offset + 18)? as usize;
            if header_entries > 65_535 {
                return Err("resource page header entry count is invalid".to_owned());
            }
            for index in 0..header_entries {
                let header = page
                    .offset
                    .checked_add(20 + index * 16)
                    .ok_or("resource page header overflow")?;
                let header_relative = 20usize
                    .checked_add(index * 16)
                    .ok_or("resource page header relative overflow")?;
                if header_relative
                    .checked_add(16)
                    .is_none_or(|end| end > page.size)
                {
                    return Err(format!(
                        "resource page header outside page page_offset=0x{:x} page_size=0x{:x} entry={index}",
                        page.offset, page.size
                    ));
                }
                let relative_resource = read_u32(&bytes, header + 8)? as usize;
                if relative_resource
                    .checked_add(16)
                    .is_none_or(|end| end > page.size)
                {
                    continue;
                }
                let absolute = page
                    .offset
                    .checked_add(relative_resource)
                    .ok_or("resource address overflow")?;
                let name_relative = read_u64(&bytes, absolute)? as usize;
                let kind_relative = read_u64(&bytes, absolute + 8)? as usize;
                let name = read_cstr(&bytes, page.offset.saturating_add(name_relative))
                    .unwrap_or_default();
                let Some(kind) = read_cstr(&bytes, page.offset.saturating_add(kind_relative))
                else {
                    continue;
                };
                if kind.is_empty() || !kind.bytes().all(|b| b.is_ascii_graphic() || b == b' ') {
                    continue;
                }
                resources.push(PakResource {
                    kind,
                    name,
                    absolute_offset: absolute,
                    page_start: page.offset,
                });
            }
        }
        if resources.is_empty() {
            return Err("TLOU2 package contains no discoverable resources".to_owned());
        }
        Ok(Self {
            bytes,
            pages,
            pointer_targets,
            resources,
            tlou2_pc,
        })
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[inline]
    pub fn resources(&self) -> &[PakResource] {
        &self.resources
    }

    /// Absolute file offset where the package VRAM region starts. TLOU2 PC stores
    /// texture payloads immediately after the final resource page; individual
    /// `VRAM_DESC` records address this region with a package-relative offset.
    pub fn vram_data_base(&self) -> Result<usize, String> {
        let page = self
            .pages
            .last()
            .ok_or_else(|| "package contains no resource pages".to_owned())?;
        page.offset
            .checked_add(page.size)
            .ok_or_else(|| "VRAM data base overflow".to_owned())
    }

    pub fn resource(&self, kind: &str) -> Option<&PakResource> {
        self.resources.iter().find(|resource| resource.kind == kind)
    }

    #[inline]
    pub fn resource_payload(&self, resource: &PakResource) -> Result<usize, String> {
        debug_assert!(self.tlou2_pc);
        let offset = resource
            .absolute_offset
            .checked_add(TLOU2_RES_PADDING)
            .ok_or("resource payload offset overflow")?;
        checked_slice(&self.bytes, offset, 1, "resource payload")?;
        Ok(offset)
    }

    pub fn resolve_pointer(&self, field: usize) -> Result<Option<usize>, String> {
        let relative = read_i64(&self.bytes, field)?;
        if relative <= 0 {
            return Ok(None);
        }
        let target_page = self.pointer_targets.get(&field).copied().ok_or_else(|| {
            format!("pointer field has no page fixup field=0x{field:x} relative=0x{relative:x}")
        })?;
        let page = self
            .pages
            .get(target_page)
            .ok_or_else(|| format!("pointer target page missing index={target_page}"))?;
        let relative = usize::try_from(relative).map_err(|_| "pointer offset exceeds usize")?;
        let absolute = page
            .offset
            .checked_add(relative)
            .ok_or("resolved pointer overflow")?;
        checked_slice(&self.bytes, absolute, 1, "resolved pointer")?;
        Ok(Some(absolute))
    }

    pub fn string_at(&self, absolute: usize) -> Result<String, String> {
        read_cstr(&self.bytes, absolute)
            .ok_or_else(|| format!("invalid source string at 0x{absolute:x}"))
    }

    pub fn read_u8(&self, offset: usize) -> Result<u8, String> {
        read_u8(&self.bytes, offset)
    }
    pub fn read_u16(&self, offset: usize) -> Result<u16, String> {
        read_u16(&self.bytes, offset)
    }
    pub fn read_u32(&self, offset: usize) -> Result<u32, String> {
        read_u32(&self.bytes, offset)
    }
    pub fn read_i32(&self, offset: usize) -> Result<i32, String> {
        read_i32(&self.bytes, offset)
    }
    pub fn read_u64(&self, offset: usize) -> Result<u64, String> {
        read_u64(&self.bytes, offset)
    }
    pub fn read_f32(&self, offset: usize) -> Result<f32, String> {
        read_f32(&self.bytes, offset)
    }
    pub fn slice(&self, offset: usize, len: usize) -> Result<&[u8], String> {
        checked_slice(&self.bytes, offset, len, "source bytes")
    }
}

fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("{label} range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| format!("{label} outside package offset=0x{offset:x} len={len}"))
}

fn read_cstr(bytes: &[u8], offset: usize) -> Option<String> {
    if offset >= bytes.len() {
        return None;
    }
    let tail = &bytes[offset..];
    let end = tail.iter().position(|byte| *byte == 0)?;
    if end == 0 || end > 16 * 1024 {
        return None;
    }
    std::str::from_utf8(&tail[..end]).ok().map(str::to_owned)
}

#[inline]
fn read_u8(bytes: &[u8], offset: usize) -> Result<u8, String> {
    Ok(*checked_slice(bytes, offset, 1, "u8")?
        .first()
        .expect("u8 slice"))
}
#[inline]
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        checked_slice(bytes, offset, 2, "u16")?
            .try_into()
            .expect("u16 slice"),
    ))
}
#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        checked_slice(bytes, offset, 4, "u32")?
            .try_into()
            .expect("u32 slice"),
    ))
}
#[inline]
fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, String> {
    Ok(i32::from_le_bytes(
        checked_slice(bytes, offset, 4, "i32")?
            .try_into()
            .expect("i32 slice"),
    ))
}
#[inline]
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        checked_slice(bytes, offset, 8, "u64")?
            .try_into()
            .expect("u64 slice"),
    ))
}
#[inline]
fn read_i64(bytes: &[u8], offset: usize) -> Result<i64, String> {
    Ok(i64::from_le_bytes(
        checked_slice(bytes, offset, 8, "i64")?
            .try_into()
            .expect("i64 slice"),
    ))
}
#[inline]
fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, String> {
    Ok(f32::from_le_bytes(
        checked_slice(bytes, offset, 4, "f32")?
            .try_into()
            .expect("f32 slice"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cstr_rejects_empty_and_out_of_range() {
        assert_eq!(read_cstr(b"abc\0", 0).as_deref(), Some("abc"));
        assert!(read_cstr(b"\0", 0).is_none());
        assert!(read_cstr(b"abc\0", 99).is_none());
    }
}
