use std::fs;
use std::io;

use directories::BaseDirs;

const DESKTOP_FILE_NAME: &str = "io.github.weversonl.GnomeBrightness.desktop";

fn autostart_path() -> Option<std::path::PathBuf> {
    let dirs = BaseDirs::new()?;
    Some(dirs.config_dir().join("autostart").join(DESKTOP_FILE_NAME))
}

pub fn is_enabled() -> bool {
    autostart_path().is_some_and(|p| p.exists())
}

pub fn enable() -> io::Result<()> {
    let path = autostart_path().ok_or_else(|| io::Error::other("no config directory"))?;
    let exe = std::env::current_exe()?;

    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=GnomeBrightness\n\
         Comment=Control external monitor brightness over DDC/CI\n\
         Exec={}\n\
         Icon=io.github.weversonl.GnomeBrightness\n\
         Terminal=false\n\
         Categories=GTK;Utility;Settings;\n\
         StartupNotify=true\n\
         X-GNOME-UsesNotifications=false\n",
        exe.display()
    );

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

pub fn disable() -> io::Result<()> {
    let Some(path) = autostart_path() else {
        return Ok(());
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
