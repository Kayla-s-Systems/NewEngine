use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone)]
pub struct ExitSignal {
    flag: Arc<AtomicBool>,
}

impl ExitSignal {
    pub fn new() -> Self {
        Self { flag: Arc::new(AtomicBool::new(false)) }
    }

    #[inline]
    pub fn request_exit(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_exit_requested(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    pub fn install_ctrlc_handler(&self) -> anyhow::Result<()> {
        let s = self.clone();
        ctrlc::set_handler(move || s.request_exit())?;
        Ok(())
    }
}