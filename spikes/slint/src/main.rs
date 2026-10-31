// Entry point for the feasibility spike. See lib.rs for the UI wiring.
fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--motion-report") {
        // Prints the OS motion reading and the value Slint receives, then exits.
        let report = onyx_slint_spike::motion_report()?;
        println!("{report}");
        return Ok(());
    }
    let app = onyx_slint_spike::create_app()?;
    slint::ComponentHandle::run(&app)
}
