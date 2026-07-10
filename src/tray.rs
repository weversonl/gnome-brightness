use gettextrs::gettext;
use ksni::menu::{StandardItem, SubMenu};
use ksni::{Icon, MenuItem, ToolTip, Tray, TrayMethods};

use crate::ddc;

pub enum TrayEvent {
    ToggleWindow,
    Detect,
    Preset(u8),
    Quit,
}

struct AppTray {
    sender: async_channel::Sender<TrayEvent>,
}

impl Tray for AppTray {
    fn icon_name(&self) -> String {
        "display-brightness-symbolic".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        Vec::new()
    }

    fn title(&self) -> String {
        gettext("Monitor Brightness")
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: gettext("Monitor Brightness"),
            ..Default::default()
        }
    }

    fn id(&self) -> String {
        "com.verso.GnomeBrightness".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.sender.try_send(TrayEvent::ToggleWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let preset = |label: String, level: u8| -> MenuItem<Self> {
            StandardItem {
                label,
                activate: Box::new(move |tray: &mut Self| {
                    let _ = tray.sender.try_send(TrayEvent::Preset(level));
                }),
                ..Default::default()
            }
            .into()
        };

        vec![
            StandardItem {
                label: gettext("Show/Hide"),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.try_send(TrayEvent::ToggleWindow);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            SubMenu {
                label: gettext("Presets"),
                submenu: vec![
                    preset("0%".into(), 0),
                    preset("25%".into(), 25),
                    preset("50%".into(), 50),
                    preset("75%".into(), 75),
                    preset("100%".into(), 100),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: gettext("Detect monitors"),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.try_send(TrayEvent::Detect);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: gettext("Quit"),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.try_send(TrayEvent::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn(sender: async_channel::Sender<TrayEvent>) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tray runtime");

        runtime.block_on(async move {
            let tray = AppTray { sender };
            if let Err(err) = tray.spawn().await {
                eprintln!("{}", gettext("Failed to start tray icon: {error}").replace("{error}", &err.to_string()));
            }
            std::future::pending::<()>().await;
        });
    });
}

pub fn apply_preset(level: u8, display_ids: &[u32]) {
    for &id in display_ids {
        std::thread::spawn(move || ddc::set_brightness(id, level));
    }
}
