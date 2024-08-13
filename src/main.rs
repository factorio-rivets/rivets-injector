mod common;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    windows::run()
}

#[cfg(target_os = "linux")]
#[allow(clippy::missing_const_for_fn)]
fn main() {}
