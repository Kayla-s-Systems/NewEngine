use newengine_model_import_northstar::{decode_vram_textures, PakFile};
use std::{env, fs};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args
        .next()
        .ok_or("usage: inspect_vram_layout PAK SELECTOR")?;
    let selector = args.next().ok_or("missing selector")?.to_ascii_lowercase();
    let pak = PakFile::parse(fs::read(&source).map_err(|e| format!("read {source}: {e}"))?)?;
    let textures = decode_vram_textures(&pak)?;
    let matches = textures
        .iter()
        .filter(|t| t.source_path.to_ascii_lowercase().contains(&selector))
        .collect::<Vec<_>>();
    let texture = match matches.as_slice() {
        [texture] => *texture,
        [] => return Err(format!("no texture contains '{selector}'")),
        many => return Err(format!("ambiguous {}", many.len())),
    };
    println!(
        "path='{}' extent={}x{} mips={} dxgi={} type={} pak_offset=0x{:x} vram_size={} stream_flags=0x{:x} absolute=0x{:x}",
        texture.source_path,
        texture.width,
        texture.height,
        texture.mip_count,
        texture.format.dxgi(),
        texture.texture_type,
        texture.pak_offset,
        texture.vram_size,
        texture.stream_flags,
        texture.absolute_data_offset,
    );
    let bpe = texture.format.bytes_per_element().ok_or("no bpe")?;
    let block = texture.format.block_extent().ok_or("no block extent")?;
    let ew = texture.width.div_ceil(block) as usize;
    let eh = texture.height.div_ceil(block) as usize;
    println!(
        "elements={}x{} bpe={} logical_base_bytes={}",
        ew,
        eh,
        bpe,
        ew * eh * bpe
    );
    let raw = pak.slice(texture.absolute_data_offset, texture.vram_size as usize)?;
    let element_count = raw.len() / bpe;
    let zero = (0..element_count)
        .filter(|i| raw[i * bpe..(i + 1) * bpe].iter().all(|&v| v == 0))
        .count();
    println!(
        "raw_elements={} raw_zero={} raw_zero_frac={:.6}",
        element_count,
        zero,
        zero as f64 / element_count.max(1) as f64
    );
    let rows = (raw.len() / bpe).div_ceil(16).min(64);
    println!("RAW first {} rows of 16 elements (#=nonzero .=zero):", rows);
    for y in 0..rows {
        let mut line = String::new();
        for x in 0..16 {
            let i = y * 16 + x;
            if i >= element_count {
                break;
            }
            let z = raw[i * bpe..(i + 1) * bpe].iter().all(|&v| v == 0);
            line.push(if z { '.' } else { '#' });
        }
        println!("{:02}: {}", y, line);
    }
    Ok(())
}
