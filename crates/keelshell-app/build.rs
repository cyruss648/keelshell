//! Embed the approved icon and version information in Windows executables.

use std::{
    env,
    error::Error,
    io,
    path::{Path, PathBuf},
};

const WINDOWS_ICON_RELATIVE_PATH: &str = "../../assets/icons/windows/keelshell.ico";
const PRODUCT_NAME: &str = "KeelShell";
const FILE_DESCRIPTION: &str = "KeelShell remote SSH manager";
const ORIGINAL_FILENAME: &str = "keelshell-app.exe";

fn windows_icon_path(manifest_directory: &Path) -> PathBuf {
    manifest_directory.join(WINDOWS_ICON_RELATIVE_PATH)
}

fn configure_windows_resource(resource: &mut winresource::WindowsResource, icon: &str) {
    // GPUI supplies resource 1/RT_MANIFEST with asInvoker and PerMonitorV2.
    // A second application manifest would collide at the Windows link step.
    resource
        .set_icon(icon)
        .set("ProductName", PRODUCT_NAME)
        .set("FileDescription", FILE_DESCRIPTION)
        .set("OriginalFilename", ORIGINAL_FILENAME);
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={WINDOWS_ICON_RELATIVE_PATH}");
    // Build scripts run on the host. The Cargo target environment is required
    // so a cross-compiled Windows binary receives its resources as well.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let directory = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| io::Error::other("Cargo manifest directory is unavailable"))?,
    );
    let icon = windows_icon_path(&directory);
    let icon = icon
        .to_str()
        .ok_or_else(|| io::Error::other("Icon path is not UTF-8"))?;
    let mut resource = winresource::WindowsResource::new();
    configure_windows_resource(&mut resource, icon);
    resource.compile()?;
    Ok(())
}
