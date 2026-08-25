use newengine_model_import_northstar::{decode_skeleton, PakFile};
use std::{env, fs};

fn score_stride(pak: &PakFile, table: usize, count: usize, stride: usize) -> usize {
    (0..count.min(64))
        .filter(|&i| {
            let sub = table + i * stride;
            pak.resolve_pointer(sub + 32)
                .ok()
                .flatten()
                .and_then(|ptr| pak.string_at(ptr).ok())
                .is_some_and(|name| name.contains("Shape") || name.contains("LOD"))
        })
        .count()
}

fn main() -> Result<(), String> {
    let mut a = env::args().skip(1);
    let source = a
        .next()
        .ok_or("usage: dump_skin_palette_candidate GEOMETRY.pak INDEX SKELETON.pak")?;
    let wanted = a
        .next()
        .ok_or("INDEX")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let skel_path = a.next().ok_or("SKELETON")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let skel_pak = PakFile::parse(fs::read(&skel_path).map_err(|e| e.to_string())?)?;
    let skel = decode_skeleton(&skel_pak)?;
    let r = pak.resource("GEOMETRY_1").ok_or("no geometry")?;
    let p = pak.resource_payload(r)?;
    let count = pak.read_u32(p + 8)? as usize;
    let table = pak.resolve_pointer(p + 40)?.ok_or("no table")?;
    let stride = if score_stride(&pak, table, count, 192) >= score_stride(&pak, table, count, 176) {
        192
    } else {
        176
    };
    let sub = table + wanted * stride;
    let name = pak
        .resolve_pointer(sub + 32)?
        .map(|x| pak.string_at(x))
        .transpose()?
        .unwrap_or_default();
    let palette = pak
        .resolve_pointer(sub + 120)?
        .ok_or("no +120 palette ptr")?;
    let count = pak.read_u32(sub + 152)? as usize;
    let aux = pak.read_u32(sub + 156)?;
    println!(
        "SUB={wanted} name='{name}' palette=0x{palette:x} count={count} aux={aux} skeleton={}",
        skel.joints.len()
    );
    println!("-- u16 interpretation --");
    for i in 0..count.min(300) {
        let v = pak.read_u16(palette + i * 2)? as usize;
        let n = skel
            .joints
            .get(v)
            .map(|j| j.name.as_str())
            .unwrap_or("<oor>");
        if i < 40
            || matches!(
                i,
                54 | 63
                    | 73
                    | 74
                    | 82
                    | 83
                    | 84
                    | 85
                    | 143
                    | 149
                    | 177
                    | 185
                    | 199
                    | 200
                    | 211
                    | 212
                    | 215
                    | 220
            )
        {
            println!("  local {i:3} -> {v:4} '{n}'");
        }
    }
    println!("-- u32 interpretation --");
    for i in 0..count.min(80) {
        let v = pak.read_u32(palette + i * 4)? as usize;
        let n = skel
            .joints
            .get(v)
            .map(|j| j.name.as_str())
            .unwrap_or("<oor>");
        if i < 40 || matches!(i, 54 | 63 | 73 | 74) {
            println!("  local {i:3} -> {v:8} '{n}'");
        }
    }
    Ok(())
}
