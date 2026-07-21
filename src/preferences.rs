use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::glib;
use libadwaita as adw;

use crate::autostart;
use crate::config::Config;
use crate::ddc;
use crate::window::apply_theme;

pub fn present(parent: &adw::ApplicationWindow, config: Rc<RefCell<Config>>, on_change: Rc<dyn Fn()>) {
    let window = adw::PreferencesWindow::builder()
        .transient_for(parent)
        .modal(true)
        .title(gettext("Preferences"))
        .build();

    let page = adw::PreferencesPage::new();

    let appearance_group = adw::PreferencesGroup::builder()
        .title(gettext("Appearance"))
        .build();

    let theme_row = adw::ComboRow::builder()
        .title(gettext("Theme"))
        .model(&gtk::StringList::new(&[
            &gettext("System"),
            &gettext("Light"),
            &gettext("Dark"),
        ]))
        .selected(theme_index(&config.borrow().theme))
        .build();

    theme_row.connect_selected_notify({
        let config = config.clone();
        move |row| {
            let theme = match row.selected() {
                1 => "light",
                2 => "dark",
                _ => "system",
            };
            let mut cfg = config.borrow_mut();
            cfg.theme = theme.to_string();
            apply_theme(&cfg);
            cfg.save();
        }
    });

    appearance_group.add(&theme_row);

    let startup_group = adw::PreferencesGroup::builder()
        .title(gettext("Startup"))
        .build();

    let autostart_row = adw::SwitchRow::builder()
        .title(gettext("Launch at login"))
        .subtitle(gettext("Start GnomeBrightness automatically when you log in"))
        .active(autostart::is_enabled())
        .build();

    autostart_row.connect_active_notify(|row| {
        let result = if row.is_active() {
            autostart::enable()
        } else {
            autostart::disable()
        };
        if let Err(err) = result {
            eprintln!("gnome-brightness: failed to update autostart: {err}");
            row.set_active(autostart::is_enabled());
        }
    });

    startup_group.add(&autostart_row);

    let minimized_row = adw::SwitchRow::builder()
        .title(gettext("Start minimized in tray"))
        .subtitle(gettext("Don't show the main window on startup"))
        .active(config.borrow().start_minimized)
        .build();

    minimized_row.connect_active_notify({
        let config = config.clone();
        move |row| {
            let mut cfg = config.borrow_mut();
            cfg.start_minimized = row.is_active();
            cfg.save();
        }
    });

    startup_group.add(&minimized_row);

    page.add(&appearance_group);
    page.add(&startup_group);

    let monitors = ddc::detect_monitors();
    if !monitors.is_empty() {
        let monitors_group = adw::PreferencesGroup::builder()
            .title(gettext("Monitors"))
            .description(gettext("Custom names are only shown in this app; they don't affect the monitor itself"))
            .build();

        for monitor in &monitors {
            let row = adw::ActionRow::builder().title(monitor.name.clone()).build();

            let entry = gtk::Entry::builder()
                .placeholder_text(gettext("Custom name"))
                .valign(gtk::Align::Center)
                .build();
            entry.set_text(config.borrow().nicknames.get(&monitor.edid_key).map_or("", String::as_str));

            let save_nickname = {
                let config = config.clone();
                let on_change = on_change.clone();
                let edid_key = monitor.edid_key.clone();
                move |entry: &gtk::Entry| {
                    let text = entry.text().to_string();
                    {
                        let mut cfg = config.borrow_mut();
                        if text.trim().is_empty() {
                            cfg.nicknames.remove(&edid_key);
                        } else {
                            cfg.nicknames.insert(edid_key.clone(), text);
                        }
                        cfg.save();
                    }
                    on_change();
                }
            };

            entry.connect_activate({
                let save_nickname = save_nickname.clone();
                move |entry| save_nickname(entry)
            });

            let focus_controller = gtk::EventControllerFocus::new();
            focus_controller.connect_leave({
                let entry = entry.clone();
                move |_| save_nickname(&entry)
            });
            entry.add_controller(focus_controller);

            row.add_suffix(&entry);
            row.set_activatable_widget(Some(&entry));
            monitors_group.add(&row);

            // Probing input sources means talking to the monitor over DDC/CI, which can
            // take a couple of seconds per display, so it's fetched off the main thread
            // and the row is filled in once the result comes back.
            let input_row = adw::ComboRow::builder()
                .title(gettext("Input source"))
                .subtitle(gettext("Detecting…"))
                .sensitive(false)
                .model(&gtk::StringList::new(&[]))
                .build();
            monitors_group.add(&input_row);

            let display_id = monitor.display_id;
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = ddc::get_input_sources_and_current(display_id);
                let _ = tx.send_blocking(result);
            });

            glib::spawn_future_local({
                let monitors_group = monitors_group.clone();
                let input_row = input_row.clone();
                async move {
                    let Ok((sources, current)) = rx.recv().await else {
                        monitors_group.remove(&input_row);
                        return;
                    };
                    if sources.is_empty() {
                        monitors_group.remove(&input_row);
                        return;
                    }

                    let labels: Vec<&str> = sources.iter().map(|(_, name)| name.as_str()).collect();
                    let selected = current
                        .and_then(|current| sources.iter().position(|(value, _)| *value == current))
                        .unwrap_or(0) as u32;

                    input_row.set_model(Some(&gtk::StringList::new(&labels)));
                    input_row.set_selected(selected);
                    input_row.set_subtitle("");
                    input_row.set_sensitive(true);

                    let values: Vec<u8> = sources.iter().map(|(value, _)| *value).collect();
                    input_row.connect_selected_notify(move |row| {
                        if let Some(&value) = values.get(row.selected() as usize) {
                            ddc::set_input_source(display_id, value);
                        }
                    });
                }
            });
        }

        page.add(&monitors_group);
    }

    window.add(&page);

    window.present();
}

fn theme_index(theme: &str) -> u32 {
    match theme {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    }
}
