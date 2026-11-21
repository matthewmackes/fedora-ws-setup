use anyhow::Result;

fn main() -> Result<()> {
    // GTK owns the process; ExitCode converted for anyhow callers.
    let code = mde_network_neighbourhood::ui::run_app();
    if code != gtk4::glib::ExitCode::SUCCESS {
        std::process::exit(1);
    }
    Ok(())
}
