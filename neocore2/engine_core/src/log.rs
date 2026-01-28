use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Logger {
    tag: &'static str,
}

/// ANSI colors
const RESET: &str = "\x1b[0m";
const GRAY: &str = "\x1b[90m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";

static BOOT: OnceLock<Instant> = OnceLock::new();

impl Logger {
    pub fn new(tag: &'static str) -> Self {
        // Единая точка отсчёта для всего процесса
        BOOT.get_or_init(Instant::now);
        Self { tag }
    }

    #[inline]
    pub fn info(&self, msg: impl AsRef<str>) {
        self.print("INFO", GREEN, msg.as_ref());
    }

    #[inline]
    pub fn debug(&self, msg: impl AsRef<str>) {
        self.print("DEBUG", CYAN, msg.as_ref());
    }

    #[inline]
    pub fn warn(&self, msg: impl AsRef<str>) {
        self.print("WARN", YELLOW, msg.as_ref());
    }

    #[inline]
    pub fn error(&self, msg: impl AsRef<str>) {
        self.print("ERROR", RED, msg.as_ref());
    }

    fn print(&self, lvl: &str, lvl_color: &str, msg: &str) {
        let boot = *BOOT.get().unwrap_or_else(|| BOOT.get_or_init(Instant::now));
        let dt = boot.elapsed();
        let stamp = fmt_uptime(dt);

        // Формат (AAA):
        // [+mm:ss.mmm] [LEVEL] [Tag] message
        println!(
            "{gray}[{stamp}]{reset} {lvl_color}[{lvl}]{reset} [{tag}] {msg}",
            gray = GRAY,
            reset = RESET,
            stamp = stamp,
            lvl_color = lvl_color,
            lvl = lvl,
            tag = self.tag,
            msg = msg
        );
    }
}

/// mm:ss.mmm (или hh:mm:ss.mmm если долго)
fn fmt_uptime(d: Duration) -> String {
    let total_ms = d.as_millis() as u64;

    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;

    let s = total_s % 60;
    let total_m = total_s / 60;

    let m = total_m % 60;
    let h = total_m / 60;

    if h > 0 {
        format!("+{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
    } else {
        format!("+{:02}:{:02}.{:03}", m, s, ms)
    }
}