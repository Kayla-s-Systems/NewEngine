pub type SystemFn = fn(&mut dyn std::any::Any);

#[derive(Clone)]
pub struct SystemEntry {
    pub order: i32,
    pub seq: u64,
    pub name: &'static str,
    pub f: SystemFn,
}