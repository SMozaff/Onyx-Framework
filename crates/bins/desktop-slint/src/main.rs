//! ONYX Desktop Slint host feasibility spike.
//!
//! The native host remains a feasibility target and is not the default desktop
//! application. Authentication wiring is introduced as a shell-independent
//! seam; this binary does not claim a live session or relay connection.

pub mod authority;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = DesktopWindow::new()?;
    ui.run()
}
