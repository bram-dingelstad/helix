use std::sync::Arc;
use std::sync::Mutex;

use libloading::{Library, Symbol};

use crate::commands::{Context, MappableCommand};

use helix_view::Editor;

pub use toml;

#[macro_export]
macro_rules! register_callbacks {
    () => {
        lazy_static! {
            static ref QUEUE_EVENT: Arc<Mutex<Option<Box<dyn FnMut(FfiCallback) + Send>>>> =
                Arc::from(Mutex::new((None)));
        }

        #[no_mangle]
        pub extern "C" fn register_event_hook(callback: RegisterCallback) {
            let mut queue_event = QUEUE_EVENT.lock().unwrap();
            let callback = unsafe { Box::from_raw(callback) };
            *queue_event = Some(callback);
        }
    };
}

#[macro_export]
macro_rules! queue_event {
    ($expression: expr) => {
        let mut queue_event = QUEUE_EVENT.lock();
        let queue_event = queue_event.as_mut().unwrap().as_mut().unwrap();

        queue_event({
            let callback = Box::from($expression);

            Box::into_raw(callback)
        });
    };
}

pub type FfiCallback = *mut (dyn FnOnce(&mut Editor) + Send);
pub type EditorCallback = Box<(dyn FnOnce(&mut Editor) + Send)>;
pub type RegisterCallback = *mut (dyn FnMut(FfiCallback) + Send);

pub enum Callback {
    Editor(EditorCallback),
}

pub struct Plugins {
    pub plugins: Vec<Arc<Mutex<Plugin>>>,
}

impl Plugins {
    pub fn new(_config: &crate::config::Config) -> Self {
        let mut plugin_config_path = helix_loader::config_dir();
        plugin_config_path.push("helix");
        plugin_config_path.set_file_name("plugins.toml");

        let plugin_config = std::fs::read_to_string(&plugin_config_path)
            .unwrap_or_default()
            .parse::<toml::Table>()
            .unwrap_or_default();

        Self {
            plugins: plugin_config
                .keys()
                .filter(|key| key.as_str() != "enabled")
                .into_iter()
                .map(|name| {
                    Plugin::new(
                        &name.replace("-", "_"),
                        plugin_config
                            .get(name)
                            .expect("to find key that we're mapping"),
                    )
                })
                .collect::<Vec<Arc<Mutex<Plugin>>>>(),
        }
    }

    pub fn deinit_hook(&self, context: &mut Context) {
        for plugin in self.plugins.iter() {
            let plugin = plugin.lock().unwrap();

            plugin.deinit_hook(context)
        }
    }

    pub async fn next_callback(&mut self) -> Option<EditorCallback> {
        for plugin in self.plugins.iter() {
            let mut plugin = plugin.lock().unwrap();

            if plugin.futures.len() > 0 {
                return plugin.futures.pop();
            }
        }
        None
    }

    pub fn get_commands(&mut self) -> Vec<MappableCommand> {
        self.plugins
            .iter()
            .map(|plugin| plugin.lock().unwrap().get_commands())
            .flatten()
            .collect::<Vec<MappableCommand>>()
    }

    pub fn handle_callback(&self, editor: &mut Editor, callback: EditorCallback) {
        callback(editor)
    }
}

pub struct Plugin {
    library: Arc<Library>,
    futures: Vec<EditorCallback>,
}

impl Plugin {
    fn new(name: &str, config: &toml::Value) -> Arc<Mutex<Self>> {
        let mut library_path = helix_loader::config_dir();
        library_path.push("plugins");
        library_path.push(format!(
            "{name}.{extension}",
            extension = match "macos" {
                "macos" => "dylib",
                _ => unimplemented!("make use of actual OS enum and cover all library extensions"),
            }
        ));

        let library = Arc::from(unsafe { Library::new(&library_path).unwrap() });

        let plugin = Arc::from(Mutex::new(Self {
            library: library.clone(),
            futures: Default::default(),
        }));

        let register_event_hook: Option<Symbol<extern "C" fn(RegisterCallback)>> =
            unsafe { library.get(b"register_event_hook").ok() };
        let init_hook: Option<Symbol<extern "C" fn(&toml::Value)>> =
            unsafe { library.get(b"init").ok() };

        if let Some(hook) = register_event_hook {
            let plugin = plugin.clone();

            hook({
                let register_hook = Box::from(move |callback: FfiCallback| {
                    let mut plugin = plugin.lock().unwrap();
                    plugin.futures.push(unsafe { Box::from_raw(callback) })
                });

                Box::into_raw(register_hook)
            });
        }

        if let Some(hook) = init_hook {
            hook(config);
        }

        plugin
    }

    fn get_commands(&self) -> Vec<MappableCommand> {
        let get_registered_command_hook: Option<Symbol<extern "C" fn() -> Vec<MappableCommand>>> =
            unsafe { self.library.get(b"register_commands").ok() };

        if let Some(hook) = get_registered_command_hook {
            hook()
        } else {
            vec![]
        }
    }

    fn deinit_hook(&self, context: &mut Context) {
        let deinit_hook: Option<Symbol<extern "C" fn(&mut crate::commands::Context)>> =
            unsafe { self.library.get(b"deinit").ok() };

        if let Some(hook) = deinit_hook {
            hook(context)
        }
    }
}
