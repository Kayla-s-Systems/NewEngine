use std::{env, fs};
use newengine_model_import_northstar::PakFile;

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args.next().ok_or("usage: inspect_vis_members <pak> [records]")?;
    let limit: usize = args.next().as_deref().unwrap_or("4").parse().map_err(|_| "invalid count")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let res = pak.resource("VIS_INFO_1").ok_or("VIS_INFO_1 missing")?;
    let base = pak.resource_payload(res)?;
    let count = pak.read_u32(base + 4)? as usize;
    let table = pak.resolve_pointer(base + 0x10)?.ok_or("record table missing")?;
    println!("VIS_MEMBERS package='{source}' count={count}");
    for index in 0..count.min(limit) {
        let rec = table + index * 0x48;
        let id = pak.read_u32(rec + 0x40)?;
        println!("REC {index} id={id}");
        let p38 = pak.resolve_pointer(rec + 0x38)?.ok_or("p38 missing")?;
        for slot in 0..6usize {
            let field = p38 + slot * 0x10;
            let n = pak.read_u32(field)? as usize;
            let ptr = pak.resolve_pointer(field + 8).ok().flatten();
            print!("  SLOT {slot} count={n}");
            if let Some(ptr) = ptr {
                print!(" ptr=0x{ptr:x} values=");
                let nread = n.min(24);
                for j in 0..nread {
                    if j != 0 { print!(","); }
                    print!("{}", pak.read_u32(ptr + j * 4).unwrap_or(u32::MAX));
                }
                if n > nread { print!(",..."); }
            }
            println!();
        }
        if let Some(p10) = pak.resolve_pointer(rec + 0x10).ok().flatten() {
            println!("  P10");
            for off in (0..0x80usize).step_by(8) {
                let a = pak.read_u32(p10 + off).unwrap_or_default();
                let b = pak.read_u32(p10 + off + 4).unwrap_or_default();
                let ptr = pak.resolve_pointer(p10 + off).ok().flatten();
                println!("    +0x{off:02x}: {a} {b} ptr={ptr:?}");
            }
        }
    }
    Ok(())
}
