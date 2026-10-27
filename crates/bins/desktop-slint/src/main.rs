//! ONYX Desktop Slint host feasibility spike.
//!
//! This executable intentionally has no Tauri dependency and no backend/session
//! wiring yet. It exists only to prove a separate native Slint binary can be
//! introduced into the workspace without changing the default desktop shell.

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = DesktopWindow::new()?;
    ui.run()
}
