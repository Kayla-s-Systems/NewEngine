use std::{collections::BTreeMap, env, fs, path::PathBuf};
use newengine_model_import_northstar::PakFile;

fn main() -> Result<(), String> {
    let root = PathBuf::from(env::args().nth(1).ok_or("usage: inventory_world_resources <pak-dir>")?);
    let mut paths=fs::read_dir(&root).map_err(|e|e.to_string())?.filter_map(Result::ok).map(|e|e.path()).filter(|p|p.extension().and_then(|s|s.to_str()).is_some_and(|s|s.eq_ignore_ascii_case("pak"))).collect::<Vec<_>>();
    paths.sort();
    let mut kinds=BTreeMap::<String,usize>::new();
    let mut parsed=0usize;
    for path in paths {
        let bytes=match fs::read(&path){Ok(v)=>v,Err(e)=>{println!("ERROR\t{}\tread\t{e}",path.display());continue;}};
        let pak=match PakFile::parse(bytes){Ok(v)=>v,Err(e)=>{println!("ERROR\t{}\tparse\t{e}",path.display());continue;}};
        parsed+=1;
        let mut local=BTreeMap::<String,usize>::new();
        for r in pak.resources(){*local.entry(r.kind.clone()).or_default()+=1;*kinds.entry(r.kind.clone()).or_default()+=1;}
        let name=path.file_name().and_then(|s|s.to_str()).unwrap_or("");
        let geometry=local.get("GEOMETRY_1").copied().unwrap_or(0);
        let collision=local.get("COLLISION_DATA_HAVOK_BG").copied().unwrap_or(0);
        let vram=local.get("VRAM_DESC").copied().unwrap_or(0);
        let level=local.get("LEVEL_INFO_3").copied().unwrap_or(0);
        let pop=local.get("POPULATOR_1").copied().unwrap_or(0);
        if geometry>0 || collision>0 || vram>0 || level>0 || pop>0 {
            println!("PAK\t{name}\tgeometry={geometry}\tcollision={collision}\tvram={vram}\tlevel={level}\tpopulator={pop}\tresources={}",pak.resources().len());
        }
    }
    println!("SUMMARY\tparsed={parsed}\tkinds={}",kinds.len());
    for (kind,count) in kinds{println!("KIND\t{kind}\t{count}");}
    Ok(())
}
