#![allow(improper_ctypes_definitions)]
use std::sync::Arc;
use std::sync::Mutex;

use lazy_static::lazy_static;

use helix_term::commands::{paste_impl, Context, MappableCommand, Paste};
use helix_term::plugins::{toml, FfiCallback, RegisterCallback};
// TODO: Maybe make seperate plugin crate?
use helix_term::{queue_event, register_callbacks};
use helix_view::{current, Editor};

register_callbacks!();

#[no_mangle]
pub extern "C" fn init(config: &toml::Value) {
    let dark_theme = config
        .get("dark_theme")
        .expect("dark_theme to be set for auto-darktheme")
        .as_str()
        .expect("dark_theme to be of type string")
        .to_owned();

    let light_theme = config
        .get("light_theme")
        .expect("light_theme to be set for auto-darktheme")
        .as_str()
        .expect("light_theme to be of type string")
        .to_owned();

    async_std::task::spawn(async move {
        let mut is_dark = false;
        let dark_theme = dark_theme.clone();
        let light_theme = light_theme.clone();

        loop {
            let value = std::process::Command::new("defaults")
                .arg("read")
                .arg("-g")
                .arg("AppleInterfaceStyle")
                .output()
                .map(|o| o.stdout.starts_with(b"Dark"))
                .unwrap_or(false);

            if value != is_dark {
                is_dark = value;

                let dark_theme = dark_theme.clone();
                let light_theme = light_theme.clone();

                queue_event!(move |editor: &mut Editor| {
                    let theme = editor
                        .theme_loader
                        .load(if is_dark { dark_theme } else { light_theme }.as_str())
                        .unwrap();

                    editor.set_theme(theme);
                });
            }
            async_std::task::sleep(std::time::Duration::from_millis(500)).await;
        }
    });
}

#[no_mangle]
pub extern "C" fn register_commands() -> Vec<MappableCommand> {
    vec![MappableCommand::Static {
        name: "hello_world",
        fun: hello_world,
        doc: "A hello world command",
    }]
}

pub fn hello_world(context: &mut Context) {
    let count = context.count();
    let (view, doc) = current!(context.editor);

    let values = &["hello world!!!! 💞".to_string()];
    paste_impl(values, doc, view, Paste::After, count, context.editor.mode);
}
