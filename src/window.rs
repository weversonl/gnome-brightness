use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{glib, Orientation};
use libadwaita as adw;
use adw::prelude::*;

use crate::config::Config;
use crate::ddc;
use crate::monitor::Monitor;

const DEBOUNCE_MS: u32 = 100;

type Debouncers = Rc<RefCell<HashMap<u32, glib::SourceId>>>;
type IndividualWidgets = Rc<RefCell<Vec<(u32, gtk::Scale, gtk::Label)>>>;

pub fn build_window(app: &adw::Application, config: Rc<RefCell<Config>>) -> adw::ApplicationWindow {
    apply_theme(&config.borrow());

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(gettext("Monitor Brightness"))
        .default_width(config.borrow().window_width)
        .default_height(config.borrow().window_height)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();

    let refresh_button = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh_button.set_tooltip_text(Some(&gettext("Detect monitors")));
    header.pack_start(&refresh_button);

    let theme_button = gtk::Button::from_icon_name(theme_icon_name(&config.borrow()));
    theme_button.set_tooltip_text(Some(&gettext("Toggle theme")));
    header.pack_end(&theme_button);

    toolbar_view.add_top_bar(&header);

    let content = gtk::Box::new(Orientation::Vertical, 18);
    content.set_margin_top(24);
    content.set_margin_bottom(20);
    content.set_margin_start(28);
    content.set_margin_end(28);

    let monitors_row = gtk::Box::new(Orientation::Horizontal, 24);
    monitors_row.set_halign(gtk::Align::Center);
    monitors_row.set_valign(gtk::Align::Start);
    monitors_row.set_vexpand(true);

    let separator = gtk::Separator::new(Orientation::Horizontal);

    let overall_row = build_overall_row();

    content.append(&monitors_row);
    content.append(&separator);
    content.append(&overall_row.container);

    toolbar_view.set_content(Some(&content));
    window.set_content(Some(&toolbar_view));

    window.connect_close_request({
        let config = config.clone();
        move |win| {
            save_window_size(win, &config);
            win.set_visible(false);
            glib::Propagation::Stop
        }
    });

    let monitors: Rc<RefCell<Vec<Monitor>>> = Rc::new(RefCell::new(Vec::new()));
    let debouncers: Debouncers = Rc::new(RefCell::new(HashMap::new()));
    let individual_widgets: IndividualWidgets = Rc::new(RefCell::new(Vec::new()));
    let programmatic: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let overall_active: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // Connected once: reconnecting per refresh would stack duplicate handlers.
    overall_row.scale.connect_value_changed({
        let monitors = monitors.clone();
        let debouncers = debouncers.clone();
        let individual_widgets = individual_widgets.clone();
        let programmatic = programmatic.clone();
        let overall_active = overall_active.clone();
        let value_label = overall_row.value_label.clone();
        let scale_for_style = overall_row.scale.clone();
        move |s| {
            if programmatic.get() {
                return;
            }
            let level = s.value().round() as u8;
            overall_active.set(true);
            set_overall_indicator(&value_label, &scale_for_style, true, level);

            programmatic.set(true);
            for (_, scale, label) in individual_widgets.borrow().iter() {
                scale.set_value(level as f64);
                label.set_text(&format!("{level}%"));
            }
            programmatic.set(false);

            for monitor in monitors.borrow_mut().iter_mut() {
                if monitor.supports_brightness {
                    monitor.value = level;
                    monitor.max_value = 100;
                    schedule_set_brightness(&debouncers, monitor.display_id, level);
                }
            }
        }
    });

    let refresh = {
        let monitors = monitors.clone();
        let monitors_row = monitors_row.clone();
        let config = config.clone();
        let debouncers = debouncers.clone();
        let individual_widgets = individual_widgets.clone();
        let programmatic = programmatic.clone();
        let overall_active = overall_active.clone();
        let overall_scale = overall_row.scale.clone();
        let overall_value_label = overall_row.value_label.clone();
        move || {
            let mut fresh = ddc::detect_monitors();
            let nicknames = &config.borrow().nicknames;
            for monitor in fresh.iter_mut() {
                if let Some(nickname) = nicknames.get(&monitor.edid_key) {
                    monitor.name = nickname.clone();
                }
            }
            *monitors.borrow_mut() = fresh;

            rebuild_monitor_columns(
                &monitors_row,
                &monitors,
                &debouncers,
                &individual_widgets,
                &programmatic,
                &overall_active,
                &overall_scale,
                &overall_value_label,
            );

            overall_active.set(false);
            let avg = average_percent(&monitors.borrow());
            programmatic.set(true);
            overall_scale.set_value(avg as f64);
            programmatic.set(false);
            set_overall_indicator(&overall_value_label, &overall_scale, false, avg);
        }
    };

    refresh();

    refresh_button.connect_clicked({
        let refresh = refresh.clone();
        move |_| refresh()
    });

    theme_button.connect_clicked({
        let config = config.clone();
        let theme_button = theme_button.clone();
        move |_| {
            {
                let mut cfg = config.borrow_mut();
                cfg.theme = next_theme(&cfg.theme);
                apply_theme(&cfg);
                cfg.save();
            }
            theme_button.set_icon_name(theme_icon_name(&config.borrow()));
        }
    });

    window
}

fn average_percent(monitors: &[Monitor]) -> u8 {
    let supported: Vec<u8> = monitors
        .iter()
        .filter(|m| m.supports_brightness)
        .map(|m| m.percent())
        .collect();
    if supported.is_empty() {
        return 0;
    }
    (supported.iter().map(|&v| v as u32).sum::<u32>() / supported.len() as u32) as u8
}

pub fn save_window_size(win: &adw::ApplicationWindow, config: &Rc<RefCell<Config>>) {
    let mut cfg = config.borrow_mut();
    cfg.window_width = win.default_width();
    cfg.window_height = win.default_height();
    cfg.save();
}

fn next_theme(current: &str) -> String {
    match current {
        "dark" => "light".to_string(),
        _ => "dark".to_string(),
    }
}

fn theme_icon_name(config: &Config) -> &'static str {
    match config.theme.as_str() {
        "dark" => "weather-clear-night-symbolic",
        _ => "weather-clear-symbolic",
    }
}

fn apply_theme(config: &Config) {
    let manager = adw::StyleManager::default();
    let scheme = match config.theme.as_str() {
        "dark" => adw::ColorScheme::ForceDark,
        "light" => adw::ColorScheme::ForceLight,
        _ => adw::ColorScheme::Default,
    };
    manager.set_color_scheme(scheme);
}

struct OverallRow {
    container: gtk::Box,
    scale: gtk::Scale,
    value_label: gtk::Label,
}

fn set_overall_indicator(value_label: &gtk::Label, scale: &gtk::Scale, active: bool, level: u8) {
    if active {
        value_label.set_text(&format!("{level}%"));
        value_label.remove_css_class("dim-label");
        scale.set_opacity(1.0);
    } else {
        value_label.set_text(&gettext("Individual"));
        value_label.add_css_class("dim-label");
        scale.set_opacity(0.55);
    }
}

fn build_overall_row() -> OverallRow {
    let container = gtk::Box::new(Orientation::Vertical, 10);

    let header_row = gtk::Box::new(Orientation::Horizontal, 8);
    let label = gtk::Label::new(Some(&gettext("Overall brightness")));
    label.set_halign(gtk::Align::Start);
    label.add_css_class("heading");
    let value_label = gtk::Label::new(Some(&gettext("Individual")));
    value_label.add_css_class("dim-label");
    value_label.set_halign(gtk::Align::End);
    value_label.set_hexpand(true);
    header_row.append(&label);
    header_row.append(&value_label);

    let slider_row = gtk::Box::new(Orientation::Horizontal, 12);
    let dim_icon = gtk::Image::from_icon_name("display-brightness-symbolic");
    let scale = gtk::Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    scale.set_hexpand(true);
    scale.set_draw_value(false);
    scale.set_opacity(0.55);
    let bright_icon = gtk::Image::from_icon_name("display-brightness-symbolic");

    slider_row.append(&dim_icon);
    slider_row.append(&scale);
    slider_row.append(&bright_icon);

    container.append(&header_row);
    container.append(&slider_row);

    OverallRow {
        container,
        scale,
        value_label,
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_monitor_columns(
    monitors_row: &gtk::Box,
    monitors: &Rc<RefCell<Vec<Monitor>>>,
    debouncers: &Debouncers,
    individual_widgets: &IndividualWidgets,
    programmatic: &Rc<Cell<bool>>,
    overall_active: &Rc<Cell<bool>>,
    overall_scale: &gtk::Scale,
    overall_value_label: &gtk::Label,
) {
    while let Some(child) = monitors_row.first_child() {
        monitors_row.remove(&child);
    }
    individual_widgets.borrow_mut().clear();

    let list = monitors.borrow();
    for monitor in list.iter() {
        let (column, scale, percent_label) = build_monitor_column(
            monitor,
            monitors.clone(),
            debouncers.clone(),
            programmatic.clone(),
            overall_active.clone(),
            overall_scale.clone(),
            overall_value_label.clone(),
        );
        monitors_row.append(&column);
        individual_widgets
            .borrow_mut()
            .push((monitor.display_id, scale, percent_label));
    }
}

#[allow(clippy::too_many_arguments)]
fn build_monitor_column(
    monitor: &Monitor,
    monitors: Rc<RefCell<Vec<Monitor>>>,
    debouncers: Debouncers,
    programmatic: Rc<Cell<bool>>,
    overall_active: Rc<Cell<bool>>,
    overall_scale: gtk::Scale,
    overall_value_label: gtk::Label,
) -> (gtk::Box, gtk::Scale, gtk::Label) {
    let column = gtk::Box::new(Orientation::Vertical, 12);
    column.set_width_request(110);
    column.set_halign(gtk::Align::Center);

    let icon = gtk::Image::from_icon_name("video-display-symbolic");
    icon.set_pixel_size(24);

    let name_label = gtk::Label::new(Some(&monitor.name));
    name_label.set_wrap(true);
    name_label.set_justify(gtk::Justification::Center);
    name_label.add_css_class("caption-heading");

    let bright_icon = gtk::Image::from_icon_name("display-brightness-symbolic");

    let scale = gtk::Scale::with_range(Orientation::Vertical, 0.0, 100.0, 1.0);
    scale.set_vexpand(true);
    scale.set_height_request(170);
    scale.set_inverted(true);
    scale.set_draw_value(false);
    scale.set_sensitive(monitor.supports_brightness);
    scale.set_value(monitor.percent() as f64);
    if !monitor.supports_brightness {
        scale.set_tooltip_text(Some(&gettext(
            "This monitor does not respond to DDC/CI commands",
        )));
    }

    let dim_icon = gtk::Image::from_icon_name("display-brightness-symbolic");

    let percent_label = gtk::Label::new(Some(&format!(
        "{}%",
        if monitor.supports_brightness { monitor.percent() } else { 0 }
    )));

    column.append(&icon);
    column.append(&name_label);
    column.append(&bright_icon);
    column.append(&scale);
    column.append(&dim_icon);
    column.append(&percent_label);

    let display_id = monitor.display_id;
    scale.connect_value_changed({
        let percent_label = percent_label.clone();
        move |s| {
            let level = s.value().round() as u8;
            percent_label.set_text(&format!("{level}%"));
            if let Some(m) = monitors
                .borrow_mut()
                .iter_mut()
                .find(|m| m.display_id == display_id)
            {
                m.value = level;
                m.max_value = 100;
            }

            if programmatic.get() {
                return;
            }

            overall_active.set(false);
            set_overall_indicator(&overall_value_label, &overall_scale, false, level);

            schedule_set_brightness(&debouncers, display_id, level);
        }
    });

    (column, scale, percent_label)
}

// Debounced so a fast slider drag doesn't flood the I2C bus.
fn schedule_set_brightness(debouncers: &Debouncers, display_id: u32, level: u8) {
    if let Some(previous) = debouncers.borrow_mut().remove(&display_id) {
        previous.remove();
    }

    let debouncers_inner = debouncers.clone();
    let source_id = glib::timeout_add_local_once(Duration::from_millis(DEBOUNCE_MS as u64), move || {
        debouncers_inner.borrow_mut().remove(&display_id);
        std::thread::spawn(move || {
            ddc::set_brightness(display_id, level);
        });
    });

    debouncers.borrow_mut().insert(display_id, source_id);
}
