use std::{env, fs};
fn main() -> Result<(), String> {
    let path = env::args().nth(1).ok_or("usage: p6_decode_ycd BODY")?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let dictionary = newengine_animation_runtime::decode_ycd_dictionary(&bytes)?;
    for clip in dictionary.clips {
        println!("clip={} duration={:.7} events={}", clip.name, clip.duration_seconds, clip.events.len());
        for event in &clip.events {
            let params = event.parameters.iter().map(|p| format!("{}={}", p.key, p.value)).collect::<Vec<_>>().join(",");
            println!("  t={:.7} tag={} params=[{}]", event.time_seconds, event.tag, params);
        }
    }
    Ok(())
}
