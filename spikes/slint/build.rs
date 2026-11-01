fn main() {
    // Debug info lets the testing backend (ElementHandle API) find elements by label.
    let config = slint_build::CompilerConfiguration::new().with_debug_info(true);
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI failed to compile");
}
