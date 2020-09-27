# `vst_window`

[![crates.io](https://img.shields.io/crates/v/vst_window.svg)](https://crates.io/crates/vst_window)
[![Docs](https://docs.rs/vst_window/badge.svg)](https://docs.rs/vst_window/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)]()

`vst_window` is a cross-platform windowing library for VST plugins, written in Rust!
You can use it to build an editor interface for a plugin using the [`vst`](https://crates.io/crates/vst) crate.

## Ideology

Full cross-platform compatibility is the top priority for this library.

There's no good reason to continue restricting users from access to high quality audio tools on the basis of their platform of choice.
Plugin developers should be able to compile and test on the machine they are most comfortable with, and be pleasantly surprised to learn that it "just works" out of the box everywhere else as well.

With that in mind, features will only be upstreamed if they are supported equally on Linux, macOS, and Windows.
However, single-platform PRs are always welcome!
Development branches for new features will be maintained until cross-platform parity is achieved.

## Features

- [x] Open and close windows using a host-provided pointer
- [x] Customizable window size
- [x] Provide a `raw-window-handle::RawWindowHandle` for rendering with popular Rust graphics backends
- [x] Recognize mouse movement
- [x] Recognize mouse buttons
  - [x] Left button press
  - [x] Left button release
  - [x] Right button press
  - [x] Right button release
  - [x] Middle button press
  - [x] Middle button release
- [ ] Recognize mouse scrolling
  - [ ] Vertical scrolling
  - [ ] Horizontal scrolling
- [ ] Recognize keyboard events
  - [ ] Key presses
  - [ ] Key releases
- [ ] Update mouse cursor
- [ ] Spawn a compatible parent window for hosts or standalone use

## Sample usage

### In the wild

[ampli-Fe](https://github.com/antonok-edm/ampli-fe) is a minimal yet complete VST plugin example, freely licensed under MIT and Apache-2.0.

### From scratch

First, it's recommended to follow the [example plugin](https://github.com/RustAudio/vst-rs#example-plugin) section from the `vst` crate to get a working plugin without an editor interface.

Be sure to add `vst_window` and [`raw-window-handle`](https://crates.io/crates/raw-window-handle) as dependencies to your `Cargo.toml` manifest.

Then, implement `vst::editor::Editor` for a new struct, such that it can build `vst_window` cross platform window handle and event source wrappers and manage them.

```rust
use core::ffi::c_void;
use vst::editor::Editor;
use vst_window::{setup, EventSource, WindowEvent};

#[derive(Default)]
struct MyPluginEditor {
    renderer: Option<MyRenderer>,
    window_events: Option<EventSource>,
}

const WINDOW_DIMENSIONS: (i32, i32) = (300, 200);

impl Editor for MyPluginEditor {
    fn size(&self) -> (i32, i32) {
        (WINDOW_DIMENSIONS.0 as i32, WINDOW_DIMENSIONS.1 as i32)
    }

    fn position(&self) -> (i32, i32) {
        (0, 0)
    }

    fn open(&mut self, parent: *mut c_void) -> bool {
        if self.window_events.is_none() {
            let (window_handle, event_source) = setup(parent, WINDOW_DIMENSIONS);
            self.renderer = Some(MyRenderer::new(window_handle));
            self.window_events = Some(event_source);
            true
        } else {
            false
        }
    }

    fn is_open(&mut self) -> bool {
        self.window_events.is_some()
    }

    fn close(&mut self) {
        drop(self.renderer.take());
        drop(self.window_events.take());
    }

    fn idle(&mut self) {
        if let Some(window_events) = &mut self.window_events {
            while let Some(event) = window_events.poll_event() {
                match event {
                    WindowEvent::MouseClick(_) => println!("Click!"),
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
    pub fn new<W: raw_window_handle::HasRawWindowHandle>(_handle: W) -> Self {
        Self
    }
    pub fn draw_frame(&mut self) {
        /* ... */
    }
}
```

Rendering code is out of the scope of this example, but there are [plenty](https://crates.io/crates/raw-window-handle/reverse_dependencies) of excellent [`raw-window-handle`](https://crates.io/crates/raw_window_handle) compatible rendering solutions.
[`wgpu`](https://crates.io/crates/wgpu) is a great choice for high-performance cross-platform rendering.

Finally, implement the `vst::plugin::Plugin::get_editor` method for your plugin.
You'll likely want to use a pattern similar to the following:

```rust
use vst::{
    editor::Editor,
    plugin::{HostCallback, Plugin},
};
use vst_window::{setup, EventSource, WindowEvent};

struct BasicPlugin {
    editor_placeholder: Option<BasicPluginEditor>,
}

impl Plugin for BasicPlugin {
    // ...

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
```

With the above code implemented, you should now be able to build and load a plugin with a blank window in your DAW.
If the DAW exposes stdout from plugins, you'll see a new "Click!" message whenever the mouse is pressed within the window.

[Full code for this sample](/examples/basic.rs) is available in the examples directory.
