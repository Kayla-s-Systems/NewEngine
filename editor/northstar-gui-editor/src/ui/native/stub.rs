use super::super::EditorStartupModel;

pub fn run(startup: &EditorStartupModel) -> Result<(), String> {
    startup.print_summary();
    println!("{}", startup.render_text());
    Ok(())
}
