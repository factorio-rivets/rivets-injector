mod common;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
#[ctor::ctor]
fn main() {
    linux::run()
}
