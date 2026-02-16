#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    cmp::Ordering,
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::logger::output::LogOutput;

pub struct LockedFileWriter {
    path: PathBuf,
    inner: Mutex<BufWriter<File>>,
}

impl LockedFileWriter {
    pub fn open_append(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let f = OpenOptions::new().create(true).append(true).open(&path)?;

        Ok(Self {
            path,
            inner: Mutex::new(BufWriter::new(f)),
        })
    }

    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Write for LockedFileWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut g = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log file mutex poisoned"))?;
        g.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        let mut g = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log file mutex poisoned"))?;
        g.flush()
    }
}

pub struct TeeWriter {
    console: LogOutput,
    file: Box<dyn Write + Send>,
}

impl TeeWriter {
    pub fn new(console: LogOutput, file: Box<dyn Write + Send>) -> Self {
        Self { console, file }
    }
}

impl Write for TeeWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.console {
            LogOutput::Stdout => io::stdout().lock().write_all(buf)?,
            LogOutput::Stderr => io::stderr().lock().write_all(buf)?,
        }
        self.file.write_all(buf)?;
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        match self.console {
            LogOutput::Stdout => io::stdout().lock().flush()?,
            LogOutput::Stderr => io::stderr().lock().flush()?,
        }
        self.file.flush()
    }
}

#[derive(Debug, Clone)]
pub struct RollingConfig {
    pub max_bytes: Option<u64>,
    pub max_files: usize,
    pub keep_days: Option<usize>,
}

/// Rolling writer (append) with:
/// - size rotation: path -> path.1 -> ... -> path.N
/// - daily rotation (UTC epoch day): path -> path.d<day> (unique), plus cleanup keep_days
pub struct RollingFileWriter {
    base_path: PathBuf,
    cfg: RollingConfig,
    inner: Mutex<RollingState>,
}

struct RollingState {
    writer: BufWriter<File>,
    size: u64,
    epoch_day: u64,
}

impl RollingFileWriter {
    pub fn open_append(base_path: impl AsRef<Path>, cfg: RollingConfig) -> io::Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        if let Some(parent) = base_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let epoch_day = utc_epoch_day(SystemTime::now());
        let (file, size) = open_file_and_size(&base_path)?;
        let state = RollingState {
            writer: BufWriter::new(file),
            size,
            epoch_day,
        };

        Ok(Self {
            base_path,
            cfg,
            inner: Mutex::new(state),
        })
    }

    fn rotate_size_locked(&self, st: &mut RollingState) -> io::Result<()> {
        st.writer.flush()?;

        rotate_numbered(&self.base_path, self.cfg.max_files)?;

        let (file, size) = open_file_and_size(&self.base_path)?;
        st.writer = BufWriter::new(file);
        st.size = size;
        Ok(())
    }

    fn rotate_daily_locked(&self, st: &mut RollingState, new_day: u64) -> io::Result<()> {
        st.writer.flush()?;

        let day_path = day_path(&self.base_path, st.epoch_day);
        let unique = unique_path(day_path);

        let _ = std::fs::rename(&self.base_path, &unique);

        if let Some(keep) = self.cfg.keep_days {
            cleanup_old_day_files(&self.base_path, keep)?;
        }

        let (file, size) = open_file_and_size(&self.base_path)?;
        st.writer = BufWriter::new(file);
        st.size = size;
        st.epoch_day = new_day;
        Ok(())
    }

    fn maybe_rotate_locked(&self, st: &mut RollingState, incoming_len: usize) -> io::Result<()> {
        let now_day = utc_epoch_day(SystemTime::now());

        if now_day != st.epoch_day {
            self.rotate_daily_locked(st, now_day)?;
        }

        if let Some(max) = self.cfg.max_bytes {
            let next = st.size.saturating_add(incoming_len as u64);
            if next > max {
                self.rotate_size_locked(st)?;
            }
        }

        Ok(())
    }
}

impl Write for RollingFileWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut st = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "rolling log mutex poisoned"))?;

        self.maybe_rotate_locked(&mut st, buf.len())?;

        let n = st.writer.write(buf)?;
        st.size = st.size.saturating_add(n as u64);
        Ok(n)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        let mut st = self
            .inner
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "rolling log mutex poisoned"))?;
        st.writer.flush()
    }
}

fn open_file_and_size(path: &Path) -> io::Result<(File, u64)> {
    let f = OpenOptions::new().create(true).append(true).open(path)?;
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    Ok((f, size))
}

fn rotate_numbered(base: &Path, max_files: usize) -> io::Result<()> {
    if max_files == 0 {
        return Ok(());
    }

    let oldest = numbered_path(base, max_files);
    let _ = std::fs::remove_file(&oldest);

    for i in (1..max_files).rev() {
        let src = numbered_path(base, i);
        let dst = numbered_path(base, i + 1);
        if src.exists() {
            let _ = std::fs::rename(src, dst);
        }
    }

    if base.exists() {
        let _ = std::fs::rename(base, numbered_path(base, 1));
    }

    Ok(())
}

fn numbered_path(base: &Path, idx: usize) -> PathBuf {
    PathBuf::from(format!("{}.{}", base.to_string_lossy(), idx))
}

fn utc_epoch_day(t: SystemTime) -> u64 {
    let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    secs / 86_400
}

fn day_path(base: &Path, day: u64) -> PathBuf {
    PathBuf::from(format!("{}.d{}", base.to_string_lossy(), day))
}

fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    for k in 1..=9999usize {
        let candidate = PathBuf::from(format!("{}.{}", path.to_string_lossy(), k));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

fn cleanup_old_day_files(base: &Path, keep_days: usize) -> io::Result<()> {
    if keep_days == 0 {
        return Ok(());
    }

    let parent = match base.parent() {
        Some(p) => p,
        None => return Ok(()),
    };

    let base_name = base
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut days: Vec<(u64, PathBuf)> = Vec::new();

    for ent in std::fs::read_dir(parent)? {
        let ent = match ent {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = ent.path();
        let name = match p.file_name().map(|s| s.to_string_lossy().to_string()) {
            Some(n) => n,
            None => continue,
        };

        if !name.starts_with(&base_name) {
            continue;
        }
        let rest = &name[base_name.len()..];
        if !rest.starts_with(".d") {
            continue;
        }
        let rest = &rest[2..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            continue;
        }
        let day = match digits.parse::<u64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        days.push((day, p));
    }

    days.sort_by(|a, b| match a.0.cmp(&b.0) {
        Ordering::Equal => a.1.cmp(&b.1),
        other => other,
    });

    if days.len() <= keep_days {
        return Ok(());
    }

    let to_remove = days.len() - keep_days;
    for (_, p) in days.into_iter().take(to_remove) {
        let _ = std::fs::remove_file(p);
    }

    Ok(())
}
