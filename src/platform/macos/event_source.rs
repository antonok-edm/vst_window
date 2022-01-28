//! Provides a source for window events on MacOS platforms.

use std::os::raw::c_void;
use std::sync::mpsc::{channel, Receiver, Sender};

use cocoa::{
    appkit::NSView,
    base::id,
    foundation::{NSPoint, NSRect, NSSize},
};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};

use super::window::EditorWindowImpl;
use crate::event::{MouseButton, WindowEvent};
use crate::platform::EventSourceBackend;

// TODO potentially move the event delegate stuff to window.rs and use the EventSubview as "the" view returned by window.rs to adhere to the true "parent" meaning

/// Name of the field used to store the `EventDelegate` pointer in the `EventSubview` class.
const EVENT_DELEGATE_IVAR: &str = "EVENT_DELEGATE_IVAR";

pub(in crate::platform) struct EventSourceImpl {
    event_subview: id,
    incoming_events: Receiver<WindowEvent>,
}

impl EventSourceBackend for EventSourceImpl {
    /// Rendering uses the host-provided NSView, but receiving window events requires a custom
    /// subclassed NSView instance. The new NSView is embedded as a subview of the original one.
    ///
    /// Events are received through overloaded method calls on the subclass. However, we can't
    /// safely access the plugin through the subclass, so we just forward them over a channel to be
    /// polled by the editor interface. The channel is part of the `EventDelegate` which is
    /// heap-allocated and pointed to by a member variable of the subclass.
    fn new(window: &EditorWindowImpl, size_xy: (i32, i32)) -> anyhow::Result<Self> {
        unsafe {
            let event_subview: id = msg_send![EVENT_SUBVIEW_DECL.class, alloc];
            event_subview.initWithFrame_(NSRect::new(
                NSPoint::new(0., 0.),
                NSSize::new(size_xy.0 as f64, size_xy.1 as f64),
            ));
            let _: () = msg_send![window.ns_view, addSubview: event_subview];

            let (event_sender, incoming_events) = channel();

            let event_delegate = EventDelegate {
                sender: event_sender,
                size_xy,
            };
            let event_delegate = Box::into_raw(Box::new(event_delegate));

            (*event_subview).set_ivar(EVENT_DELEGATE_IVAR, event_delegate as *mut c_void);

            Ok(Self {
                event_subview,
                incoming_events,
            })
        }
    }

    fn poll_event(&self) -> Option<WindowEvent> {
        match self.incoming_events.try_recv() {
            Ok(ev) => Some(ev),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => unreachable!(
                "self.event_subview is released when self is dropped, panic should abort"
            ),
        }
    }
}

impl Drop for EventSourceImpl {
    fn drop(&mut self) {
        let _: id = unsafe { msg_send![self.event_subview, release] };
    }
}

/// Stored within the `EventSubview` class to support sending events back to the cross-platform
/// `EventSourceImpl` abstraction from Objective-C callbacks.
struct EventDelegate {
    sender: Sender<WindowEvent>,
    size_xy: (i32, i32),
}

impl EventDelegate {
    /// Returns a mutable reference to an EventDelegate from an Objective-C callback.
    ///
    /// `clippy` has issues with this function signature, making the valid point that this could
    /// create multiple mutable references to the `EventDelegate`. However, in practice macOS
    /// blocks for the entire duration of each event callback, so this should be fine.
    //#[allow(clippy::mut_from_ref)]
    fn from_field(obj: &Object) -> &mut EventDelegate {
        unsafe {
            let delegate_ptr: *mut c_void = *obj.get_ivar(EVENT_DELEGATE_IVAR);
            &mut *(delegate_ptr as *mut EventDelegate)
        }
    }

    /// Convenience method to avoid `delegate.sender.send(...).unwrap()` boilerplate.
    fn send(&mut self, event: WindowEvent) {
        self.sender.send(event).unwrap();
    }
}

/// Typesafe wrapper around the dynamic Objective-C `Class` type specific to the
/// `EVENT_SUBVIEW_DECL`.
struct EventSubview {
    class: *const Class,
}
unsafe impl Send for EventSubview {}
unsafe impl Sync for EventSubview {}

/// Lazily initialized NSView subclass declaration that is capable of receiving window events
/// through overloaded methods. Crucially, it holds an `EventDelegate` pointer so it can forward
/// events back to the editor logic.
lazy_static::lazy_static! {
    static ref EVENT_SUBVIEW_DECL: EventSubview = unsafe {
        let mut class = ClassDecl::new("EventSubview", class!(NSView)).unwrap();
        class.add_method(sel!(dealloc), dealloc as extern "C" fn(&Object, Sel));
        class.add_method(
            sel!(mouseDown:),
            mouse_down as extern "C" fn(&Object, Sel, id),
        );
        class.add_method(sel!(mouseUp:), mouse_up as extern "C" fn(&Object, Sel, id));
        class.add_method(
            sel!(rightMouseDown:),
            right_mouse_down as extern "C" fn(&Object, Sel, id),
        );
        class.add_method(
            sel!(rightMouseUp:),
            right_mouse_up as extern "C" fn(&Object, Sel, id),
        );
        class.add_method(
            sel!(otherMouseDown:),
            other_mouse_down as extern "C" fn(&Object, Sel, id),
        );
        class.add_method(
            sel!(otherMouseUp:),
            other_mouse_up as extern "C" fn(&Object, Sel, id),
        );
        class.add_method(
            sel!(mouseMoved:),
            mouse_moved as extern "C" fn(&Object, Sel, id),
        );
        class.add_method(
            sel!(mouseDragged:),
            mouse_dragged as extern "C" fn(&Object, Sel, id),
        );
        class.add_ivar::<*mut c_void>(EVENT_DELEGATE_IVAR);
        EventSubview {
            class: class.register(),
        }
    };
}

extern "C" fn dealloc(this: &Object, _sel: Sel) {
    unsafe {
        let delegate_ptr: *mut c_void = *this.get_ivar(EVENT_DELEGATE_IVAR);
        Box::from_raw(delegate_ptr as *mut EventDelegate);
    }
}

extern "C" fn mouse_down(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate = EventDelegate::from_field(this);

    delegate.send(WindowEvent::CursorMovement(
        (location.x / delegate.size_xy.0 as f64) as f32,
        1. - (location.y / delegate.size_xy.1 as f64) as f32,
    ));
    delegate.send(WindowEvent::MouseClick(MouseButton::Left));
}

extern "C" fn mouse_up(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate = EventDelegate::from_field(this);

    delegate.send(WindowEvent::CursorMovement(
        (location.x / delegate.size_xy.0 as f64) as f32,
        1. - (location.y / delegate.size_xy.1 as f64) as f32,
    ));
    delegate.send(WindowEvent::MouseRelease(MouseButton::Left));
}

extern "C" fn right_mouse_down(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate = EventDelegate::from_field(this);

    delegate.send(WindowEvent::CursorMovement(
        (location.x / delegate.size_xy.0 as f64) as f32,
        1. - (location.y / delegate.size_xy.1 as f64) as f32,
    ));
    delegate.send(WindowEvent::MouseClick(MouseButton::Right));

    // TODO potentially call super https://developer.apple.com/documentation/appkit/nsview
}

extern "C" fn right_mouse_up(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate = EventDelegate::from_field(this);

    delegate.send(WindowEvent::CursorMovement(
        (location.x / delegate.size_xy.0 as f64) as f32,
        1. - (location.y / delegate.size_xy.1 as f64) as f32,
    ));
    delegate.send(WindowEvent::MouseRelease(MouseButton::Right));
}

extern "C" fn other_mouse_down(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate = EventDelegate::from_field(this);

    delegate.send(WindowEvent::CursorMovement(
        (location.x / delegate.size_xy.0 as f64) as f32,
        1. - (location.y / delegate.size_xy.1 as f64) as f32,
    ));
    delegate.send(WindowEvent::MouseClick(MouseButton::Middle));
}

extern "C" fn other_mouse_up(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate = EventDelegate::from_field(this);

    delegate.send(WindowEvent::CursorMovement(
        (location.x / delegate.size_xy.0 as f64) as f32,
        1. - (location.y / delegate.size_xy.1 as f64) as f32,
    ));
    delegate.send(WindowEvent::MouseRelease(MouseButton::Middle));
}

extern "C" fn mouse_moved(this: &Object, _sel: Sel, event: id) {
    let location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
    let delegate = EventDelegate::from_field(this);

    delegate.send(WindowEvent::CursorMovement(
        (location.x / delegate.size_xy.0 as f64) as f32,
        1. - (location.y / delegate.size_xy.1 as f64) as f32,
    ));
}

extern "C" fn mouse_dragged(this: &Object, sel: Sel, event: id) {
    mouse_moved(this, sel, event)
}
