#[derive(Clone, Debug, Default)]
pub struct AssetsAdapterTrace {
    pub gateway: &'static str,
    pub method: &'static str,
    pub request_ref: Option<String>,
    pub diagnostic: Option<String>,
}
