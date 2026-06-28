#[cfg(not(target_os = "windows"))]
mod stub;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_os = "windows"))]
pub(super) use stub::run;
#[cfg(target_os = "windows")]
pub(super) use windows::run;
