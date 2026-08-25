use newengine_model_import_northstar::PakFile;
use std::{collections::BTreeMap, env, fs};
fn main() -> Result<(), String> {
    for s in env::args().skip(1) {
        let p = PakFile::parse(fs::read(&s).map_err(|e| e.to_string())?)?;
        let mut m = BTreeMap::<u32, usize>::new();
        for r in p.resources().iter().filter(|r| r.kind == "VRAM_DESC") {
            let q = p.resource_payload(r)?;
            *m.entry(p.read_u32(q + 0x28)?).or_default() += 1;
        }
        println!("{} {:?}", s, m);
    }
    Ok(())
}
