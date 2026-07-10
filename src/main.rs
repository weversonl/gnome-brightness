mod config;
mod ddc;
mod monitor;
mod tray;
mod window;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};
use gtk::{gio, glib};
use libadwaita as adw;
use adw::prelude::*;

const APP_ID: &str = "com.verso.GnomeBrightness";

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
    init_i18n();

    let app = adw::Application::builder().application_id(APP_ID).build();

    let config = Rc::new(RefCell::new(config::Config::load()));

    app.connect_activate(move |app| {
        let window = Rc::new(window::build_window(app, config.clone()));

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

    app.run()
}
