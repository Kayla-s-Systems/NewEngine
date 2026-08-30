use newengine_model_import_northstar::{decode_vram_textures, PakFile};
use std::{env, fs, path::PathBuf};

fn write_tga(path: &PathBuf, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected { return Err(format!("rgba bytes={} expected={expected}", rgba.len())); }
    let mut out = Vec::with_capacity(18 + expected);
    out.extend_from_slice(&[0,0,2]); out.extend_from_slice(&[0;5]); out.extend_from_slice(&[0;4]);
    out.extend_from_slice(&(width as u16).to_le_bytes()); out.extend_from_slice(&(height as u16).to_le_bytes());
    out.push(32); out.push(0x28);
    for px in rgba.chunks_exact(4) { out.extend_from_slice(&[px[2],px[1],px[0],px[3]]); }
    if let Some(parent)=path.parent(){ fs::create_dir_all(parent).map_err(|e|e.to_string())?; }
    fs::write(path,out).map_err(|e|e.to_string())?; Ok(())
}

fn main() -> Result<(), String> {
    let mut args=env::args().skip(1);
    let pak_path=PathBuf::from(args.next().ok_or("usage: batch PAK OUTDIR selector...")?);
    let out_dir=PathBuf::from(args.next().ok_or("missing out dir")?);
    let selectors=args.collect::<Vec<_>>();
    if selectors.is_empty(){return Err("no selectors".to_owned());}
    eprintln!("reading {}",pak_path.display());
    let bytes=fs::read(&pak_path).map_err(|e|format!("read {}: {e}",pak_path.display()))?;
    eprintln!("parsing {} bytes",bytes.len());
    let pak=PakFile::parse(bytes)?;
    let textures=decode_vram_textures(&pak)?;
    eprintln!("textures={}",textures.len());
    for selector in selectors {
        let needle=selector.to_ascii_lowercase();
        let matches=textures.iter().filter(|t|t.source_path.to_ascii_lowercase().contains(&needle)).collect::<Vec<_>>();
        let texture=match matches.as_slice(){[t]=>*t,[]=>return Err(format!("no match {selector}")),many=>return Err(format!("ambiguous {selector}: {}",many.iter().map(|t|t.source_path.as_str()).collect::<Vec<_>>().join(" | ")))};
        let rgba=texture.base_rgba8(&pak)?;
        let safe=selector.replace('/',"_").replace('\\',"_");
        let out=out_dir.join(format!("{safe}.tga"));
        write_tga(&out,texture.width,texture.height,&rgba)?;
        println!("EXTRACT selector='{selector}' source='{}' {}x{} format={:?} output='{}'",texture.source_path,texture.width,texture.height,texture.format,out.display());
    }
    Ok(())
}
