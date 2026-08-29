use std::{env, fs};
use newengine_model_import_northstar::PakFile;

fn dump(pak: &PakFile, label: &str, at: usize, bytes: usize) {
    println!("{label} at=0x{at:x}");
    for off in (0..bytes).step_by(4) {
        let u = pak.read_u32(at + off).unwrap_or_default();
        let f = pak.read_f32(at + off).unwrap_or(f32::NAN);
        let ptr = if off % 8 == 0 { pak.resolve_pointer(at + off).ok().flatten() } else { None };
        let string = ptr.and_then(|p| pak.string_at(p).ok()).filter(|s| !s.is_empty() && s.len() < 400 && s.bytes().all(|b| b.is_ascii_graphic() || b == b' '));
        println!("  +0x{off:03x} u={u:10} f={f:14.6} ptr={ptr:?} str={string:?}");
    }
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args.next().ok_or("usage: inspect_geometry_links <pak> [instance-index]")?;
    let index: usize = args.next().as_deref().unwrap_or("0").parse().map_err(|_| "invalid index")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let res = pak.resource("GEOMETRY_1").ok_or("GEOMETRY_1 missing")?;
    let base = pak.resource_payload(res)?;
    let table_b = pak.resolve_pointer(base + 0x38)?.ok_or("table B missing")?;
    let inst = pak.resolve_pointer(table_b + index * 8)?.ok_or("instance missing")?;
    dump(&pak, "INSTANCE", inst, 0x110);
    for (name, off, size) in [("DEF",0x80,0xb0usize),("B90",0x90,0x100),("B98",0x98,0x100)] {
        if let Some(p) = pak.resolve_pointer(inst + off).ok().flatten() {
            dump(&pak, name, p, size);
            if name == "DEF" {
                for (sub, soff) in [("DEF40",0x40usize),("DEF30",0x30usize)] {
                    if let Some(q)=pak.resolve_pointer(p+soff).ok().flatten(){dump(&pak,sub,q,0x100);}
                }
            }
        }
    }
    Ok(())
}
