mod autostart;
mod config;
mod ddc;
mod monitor;
mod preferences;
mod tray;
mod window;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};
use gtk::{gio, glib};
use gtk::prelude::ApplicationExtManual;
use libadwaita as adw;
use adw::prelude::*;

const APP_ID: &str = "io.github.weversonl.GnomeBrightness";

fn wants_verbose() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == "-v" || arg == "--verbose")
}

/// Forks to background and detaches from the controlling terminal so the
/// shell that launched the app is freed immediately, mirroring how apps
/// started from the GNOME launcher behave. Skipped when --verbose is passed.
fn daemonize() {
    use std::os::unix::io::AsRawFd;

    unsafe {
        match libc::fork() {
            pid if pid < 0 => eprintln!("gnome-brightness: fork failed, staying in foreground"),
            0 => {
                libc::setsid();
                if let Ok(dev_null) = std::fs::OpenOptions::new().read(true).write(true).open("/dev/null") {
                    let fd = dev_null.as_raw_fd();
                    libc::dup2(fd, 0);
                    libc::dup2(fd, 1);
                    libc::dup2(fd, 2);
                }
            }
            _ => std::process::exit(0),
        }
    }
}

fn init_i18n() {
    setlocale(LocaleCategory::LcAll, "");
    let domain = "gnome-brightness";

    let installed_locale_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().and_then(|p| p.parent()).map(|p| p.join("share/locale")));
    let dev_locale_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("po/locale");

    let locale_dir = match installed_locale_dir {
        Some(dir) if dir.exists() => dir,
        _ => dev_locale_dir,
    };

    let _ = bindtextdomain(domain, locale_dir);
    let _ = bind_textdomain_codeset(domain, "UTF-8");
    let _ = textdomain(domain);
}

fn main() -> glib::ExitCode {
    if !wants_verbose() {
        daemonize();
    }

    init_i18n();

    let app = adw::Application::builder().application_id(APP_ID).build();

    let config = Rc::new(RefCell::new(config::Config::load()));
    let window_slot: Rc<RefCell<Option<Rc<adw::ApplicationWindow>>>> = Rc::new(RefCell::new(None));

    app.connect_activate(move |app| {
        if let Some(window) = window_slot.borrow().as_ref() {
            window.present();
            return;
        }

        let (window, refresh) = window::build_window(app, config.clone());
        let window = Rc::new(window);
        *window_slot.borrow_mut() = Some(window.clone());

        if !config.borrow().start_minimized {
            window.present();
        }

        app.connect_shutdown({
            let window = window.clone();
            let config = config.clone();
            move |_| window::save_window_size(&window, &config)
        });

        let (sender, receiver) = async_channel::unbounded::<tray::TrayEvent>();
        tray::spawn(sender);

        let window_for_tray = window.clone();
        let app_for_tray = app.clone();
        let config_for_tray = config.clone();
        let refresh_for_tray = refresh.clone();
        glib::spawn_future_local(async move {
            while let Ok(event) = receiver.recv().await {
                match event {
                    tray::TrayEvent::ToggleWindow => {
                        if window_for_tray.is_visible() {
                            window_for_tray.set_visible(false);
                        } else {
                            window_for_tray.present();
                        }
                    }
                    tray::TrayEvent::Detect => {
                        window_for_tray.present();
                    }
                    tray::TrayEvent::Preferences => {
                        window_for_tray.present();
                        preferences::present(
                            &window_for_tray,
                            config_for_tray.clone(),
                            refresh_for_tray.clone(),
                        );
                    }
                    tray::TrayEvent::Preset(level) => {
                        let ids: Vec<u32> = ddc::detect_monitors()
                            .into_iter()
                            .filter(|m| m.supports_brightness)
                            .map(|m| m.display_id)
                            .collect();
                        tray::apply_preset(level, &ids);
                    }
                    tray::TrayEvent::Quit => {
                        app_for_tray.quit();
                    }
                }
            }
        });
    });

    app.set_flags(gio::ApplicationFlags::empty());
    let _hold_guard = app.hold();

    let gtk_args: Vec<String> = std::env::args()
        .filter(|arg| arg != "-v" && arg != "--verbose")
        .collect();
    app.run_with_args(&gtk_args)
}
