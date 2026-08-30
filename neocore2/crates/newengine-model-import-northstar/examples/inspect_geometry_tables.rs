use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn inspect_record(pak: &PakFile, label: &str, at: usize, size: usize) {
    println!("{label} at=0x{at:x} size=0x{size:x}");
    for off in (0..size).step_by(4) {
        let u = pak.read_u32(at + off).unwrap_or_default();
        let f = pak.read_f32(at + off).unwrap_or(f32::NAN);
        let ptr = if off % 8 == 0 {
            pak.resolve_pointer(at + off).ok().flatten()
        } else {
            None
        };
        let string = ptr.and_then(|p| pak.string_at(p).ok()).filter(|s| {
            !s.is_empty() && s.len() < 300 && s.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
        });
        if ptr.is_some()
            || string.is_some()
            || (f.is_finite() && f.abs() >= 0.00001 && f.abs() < 100000.0)
            || u < 100000
        {
            println!("  +0x{off:03x} u={u:10} f={f:14.6} ptr={ptr:?} str={string:?}");
        }
    }
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args
        .next()
        .ok_or("usage: inspect_geometry_tables <pak> [records]")?;
    let limit: usize = args
        .next()
        .as_deref()
        .unwrap_or("3")
        .parse()
        .map_err(|_| "invalid count")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let res = pak.resource("GEOMETRY_1").ok_or("GEOMETRY_1 missing")?;
    let base = pak.resource_payload(res)?;
    let count_a = pak.read_u32(base + 0x0c)? as usize;
    let count_b = pak.read_u32(base + 0x10)? as usize;
    let table_a = pak.resolve_pointer(base + 0x30)?.ok_or("table A missing")?;
    let table_b = pak.resolve_pointer(base + 0x38)?.ok_or("table B missing")?;
    println!("GEOMETRY_TABLES package='{source}' count_a={count_a} table_a=0x{table_a:x} count_b={count_b} table_b=0x{table_b:x}");
    for i in 0..count_a.min(limit) {
        if let Some(at) = pak.resolve_pointer(table_a + i * 8).ok().flatten() {
            inspect_record(&pak, &format!("A[{i}]"), at, 0xb0);
        }
    }
    for i in 0..count_b.min(limit) {
        if let Some(at) = pak.resolve_pointer(table_b + i * 8).ok().flatten() {
            inspect_record(&pak, &format!("B[{i}]"), at, 0x110);
        }
    }
    Ok(())
}
