//! Embed the approved icon and version information in Windows executables.

use std::{env, error::Error, io, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=../../assets/icons/windows/keelshell.ico");
    // Build scripts run on the host. The Cargo target environment is required
    // so a cross-compiled Windows binary receives its resources as well.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let directory = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| io::Error::other("Cargo manifest directory is unavailable"))?,
    );
    let icon = directory.join("../../assets/icons/windows/keelshell.ico");
    let icon = icon
        .to_str()
        .ok_or_else(|| io::Error::other("Icon path is not UTF-8"))?;
    let mut resource = winresource::WindowsResource::new();
    // GPUI supplies resource 1/RT_MANIFEST with asInvoker and PerMonitorV2.
    // A second application manifest would collide at the Windows link step.
    resource
        .set_icon(icon)
        .set("ProductName", "KeelShell")
        .set("FileDescription", "KeelShell remote SSH manager")
        .set("OriginalFilename", "keelshell-app.exe");
    resource.compile()?;
    Ok(())
}
