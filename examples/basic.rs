//! Minimal VST plugin with an editor window.
//!
//! The editor window is blank. Clicking anywhere in the window will print "Click!" to stdout.

use core::ffi::c_void;
use vst::{
    editor::Editor,
    plugin::{HostCallback, Info, Plugin},
    plugin_main,
};
use vst_window::{setup, EditorWindow, WindowEvent};

#[derive(Default)]
struct BasicPlugin {
    editor_placeholder: Option<MyPluginEditor>,
}

impl Plugin for BasicPlugin {
    fn get_info(&self) -> Info {
        Info {
            name: "Basic Plugin with Editor".to_string(),
            unique_id: 13579,

            ..Default::default()
        }
    }

    fn new(_host: HostCallback) -> Self {
        Self {
            editor_placeholder: Some(MyPluginEditor::default()),
        }
    }

    fn get_editor(&mut self) -> Option<Box<dyn Editor>> {
        self.editor_placeholder
            .take()
            .map(|editor| Box::new(editor) as Box<dyn Editor>)
    }
}

plugin_main!(BasicPlugin);

#[derive(Default)]
struct MyPluginEditor {
    renderer: Option<MyRenderer>,
    window: Option<EditorWindow>,
}

const WINDOW_DIMENSIONS: (i32, i32) = (300, 200);

impl Editor for MyPluginEditor {
    fn size(&self) -> (i32, i32) {
        (WINDOW_DIMENSIONS.0, WINDOW_DIMENSIONS.1)
    }

    fn position(&self) -> (i32, i32) {
        (0, 0)
    }

    fn open(&mut self, parent: *mut c_void) -> bool {
        if self.window.is_none() {
            match unsafe { setup(parent, WINDOW_DIMENSIONS) } {
                Ok(window) => {
                    self.renderer = Some(MyRenderer::new(&window));
                    self.window = Some(window);
                    true
                }
                Err(error) => {
                    log::error!("Failed to open editor window: {}", error);
                    false
                }
            }
        } else {
            false
        }
    }

    fn is_open(&mut self) -> bool {
        self.window.is_some()
    }

    fn close(&mut self) {
        drop(self.renderer.take());
        drop(self.window.take());
    }

    fn idle(&mut self) {
        if let Some(window) = &mut self.window {
            while let Some(event) = window.poll_event() {
                match event {
                    WindowEvent::MouseClick(_) => println!("Click!"),
                    WindowEvent::MouseRelease(_) => println!("Clack!"),
                    _ => (),
                }
            }
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.draw_frame();
        }
    }
}

struct MyRenderer;

impl MyRenderer {
    pub fn new<W: raw_window_handle::HasRawWindowHandle>(_handle: &W) -> Self {
        Self
    }
    pub fn draw_frame(&mut self) {
        /* ... */
    }
}
