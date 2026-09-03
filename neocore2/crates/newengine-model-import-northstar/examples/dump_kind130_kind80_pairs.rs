use newengine_model_import_northstar::PakFile;
use std::{env, fs, io::Write};

#[derive(Clone, Copy)]
struct S {
    kind: u8,
    buf: usize,
    len: usize,
    sz: [u8; 4],
    sc: [f32; 4],
    off: [f32; 4],
}
struct BR<'a> {
    d: &'a [u8],
    b: usize,
}
impl<'a> BR<'a> {
    fn r(&mut self, w: usize) -> u32 {
        let mut v = 0;
        for i in 0..w {
            let a = self.b + i;
            v |= (((self.d[a >> 3] >> (a & 7)) & 1) as u32) << i;
        }
        self.b += w;
        v
    }
}
fn sd(p: &PakFile, a: usize) -> Result<S, String> {
    let mut sc = [0.; 4];
    let mut off = [0.; 4];
    for i in 0..4 {
        sc[i] = p.read_f32(a + 32 + i * 4)?;
        off[i] = p.read_f32(a + 48 + i * 4)?;
    }
    Ok(S {
        kind: p.read_u8(a + 20)?,
        buf: p.resolve_pointer(a)?.ok_or("buf")?,
        len: p.read_u32(a + 16)? as usize,
        sz: [
            p.read_u8(a + 24)?,
            p.read_u8(a + 25)?,
            p.read_u8(a + 26)?,
            p.read_u8(a + 27)?,
        ],
        sc,
        off,
    })
}
fn qdec(p: &PakFile, s: S, n: usize) -> Result<Vec<[f32; 4]>, String> {
    let d = p.slice(s.buf, s.len)?;
    let mut br = BR { d, b: 0 };
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut v = [0.; 4];
        for (c, value) in v.iter_mut().enumerate() {
            let w = s.sz[c] as usize;
            if w > 0 {
                *value = br.r(w) as f32 * s.sc[c] + s.off[c];
            }
        }
        out.push(v)
    }
    Ok(out)
}
fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let src = args.next().ok_or("pak")?;
    let output = args.next().ok_or("output")?;
    let pak = PakFile::parse(fs::read(src).map_err(|e| e.to_string())?)?;
    let r = pak.resource("GEOMETRY_1").ok_or("geom")?;
    let p = pak.resource_payload(r)?;
    let count = pak.read_u32(p + 8)? as usize;
    let table = pak.resolve_pointer(p + 40)?.ok_or("table")?;
    let mut rows = Vec::<(u32, [f32; 3])>::new();
    for mi in 0..count {
        let sm = table + mi * 192;
        let name = pak
            .resolve_pointer(sm + 32)?
            .map(|x| pak.string_at(x))
            .transpose()?
            .unwrap_or_default();
        if !name.contains("LODShape0") {
            continue;
        }
        let vc = pak.read_u32(sm + 136)? as usize;
        let sc = pak.read_u32(sm + 144)? as usize;
        let st = pak.resolve_pointer(sm + 48)?.ok_or("streams")?;
        let ss = (0..sc)
            .map(|j| sd(&pak, st + j * 64))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(s80) = ss
            .iter()
            .find(|s| s.kind == 80 && s.sz[0] > 0 && s.sz[1] > 0 && s.sz[2] > 0)
            .copied()
        else {
            continue;
        };
        let Some(s130) = ss.iter().find(|s| s.kind == 130).copied() else {
            continue;
        };
        let gt = qdec(&pak, s80, vc)?;
        let raw = pak.slice(s130.buf, vc * 4)?;
        for (packed, tangent) in raw.as_chunks::<4>().0.iter().zip(gt.iter()).take(vc) {
            rows.push((
                u32::from_le_bytes(*packed),
                [tangent[0], tangent[1], tangent[2]],
            ));
        }
        println!("collected mesh={mi} vertices={vc} name='{name}'");
    }
    if rows.is_empty() {
        return Err("no overlapping kind130/kind80 meshes".into());
    }
    let mut f = fs::File::create(output).map_err(|e| e.to_string())?;
    writeln!(f, "raw,nx,ny,nz").map_err(|e| e.to_string())?;
    for (raw, n) in rows {
        writeln!(f, "{raw},{},{},{}", n[0], n[1], n[2]).map_err(|e| e.to_string())?;
    }
    Ok(())
}
