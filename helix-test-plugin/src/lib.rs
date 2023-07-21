#![allow(improper_ctypes_definitions)]
use futures::lock::Mutex;
use helix_term::commands::{paste_impl, Context, MappableCommand, Paste};
use helix_view::current;
use lazy_static::lazy_static;
use std::sync::Arc;

use helix_term::tokio;

#[no_mangle]
pub extern "C" fn register_commands() -> Vec<MappableCommand> {
    vec![MappableCommand::Static {
        name: "hello_world",
        fun: command_hello_world,
        doc: "A hello world command",
    }]
}

lazy_static! {
    static ref IS_DARK: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref RUNTIME: tokio::runtime::Runtime = tokio::runtime::Runtime::new().unwrap();
}

#[no_mangle]
pub extern "C" fn init(context: &mut Context<'_>) {
    let _runtime = RUNTIME.enter();

    tokio::spawn(async move {
        loop {
            let mut is_dark = IS_DARK.lock().await;

            *is_dark = std::process::Command::new("defaults")
                .arg("read")
                .arg("-g")
                .arg("AppleInterfaceStyle")
                .output()
                .map(|o| o.stdout.starts_with(b"Dark"))
                .unwrap();
            drop(is_dark);

            // println!("🤔 {:?}", is_dark);
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }
    });

    set_theme_based_on_mode(context.editor);
}

#[no_mangle]
pub extern "C" fn deinit() {}

pub fn command_hello_world(context: &mut Context) {
    let _runtime = RUNTIME.enter();

    let count = context.count();
    let (view, doc) = current!(context.editor);

    let values = &["hello world!   !!! 💞".to_string()];
    paste_impl(values, doc, view, Paste::After, count, context.editor.mode);
}

#[no_mangle]
pub extern "C" fn render(context: &mut helix_term::compositor::Context<'_>) {
    set_theme_based_on_mode(context.editor);
}

fn set_theme_based_on_mode(editor: &mut helix_view::Editor) {
    let is_dark = RUNTIME.block_on(async { *IS_DARK.lock().await });
    let theme_name = if is_dark {
        "rose_pine"
    } else {
        "rose_pine_dawn"
    };

    let theme = editor.theme_loader.load(theme_name).unwrap();

    editor.set_theme(theme);
}
