use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let po_dir = Path::new(&manifest_dir).join("po");
    let locale_dir = po_dir.join("locale");

    println!("cargo:rerun-if-changed={}", po_dir.display());

    let Ok(entries) = std::fs::read_dir(&po_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("po") {
            continue;
        }
        let Some(lang) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };

        let mo_dir = locale_dir.join(&lang).join("LC_MESSAGES");
        if std::fs::create_dir_all(&mo_dir).is_err() {
            continue;
        }
        let mo_path = mo_dir.join("gnome-brightness.mo");

        let _ = Command::new("msgfmt").arg(&path).arg("-o").arg(&mo_path).status();
    }
}
