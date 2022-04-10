//! Implementation of objective-c class `EventProxyView`.

use std::os::raw::c_void;
use std::sync::mpsc::Sender;

use cocoa::{
    base::id,
    foundation::{NSPoint, NSRect, NSSize},
};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    rc::StrongPtr,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};

use crate::{SetupError, WindowEvent};

/// Name of the instance variable used to store the owned `EventDelegate` pointer in the `EventSubview` objective-c class.
const EVENT_DELEGATE_IVAR: &str = "EVENT_DELEGATE_IVAR";
/// Type of the instance variable used to store the owned `EventDelegate` pointer in the `EventSubview` objective-c class.
/// This is declared here to comply with the safety requirements of [objc::runtime::Object::get_ivar] et al.
type EventDelegateIvarType = *mut c_void;

/// Instantiate an objective-c `EventProxyView` object.
///
/// `EventProxyView` is a subclass of `NSView` and proxies window events to the given `event_sender`.
/// `size_xy` is the size of the `NSView`.
/// The newly created view will be added as a subview to `parent`.
///
/// # Safety
/// `parent` must be a valid objective-c object.
pub unsafe fn instantiate_event_proxy(
    parent: id,
    event_sender: Sender<WindowEvent>,
    size_xy: (i32, i32),
) -> Result<StrongPtr, SetupError> {
    let event_proxy_view: id = unsafe { msg_send![*EVENT_PROXY_VIEW_CLASS, alloc] };
    let frame = NSRect::new(
        NSPoint::new(0., 0.),
        NSSize::new(size_xy.0 as f64, size_xy.1 as f64),
    );

    let event_delegate = EventDelegate {
        sender: event_sender,
        size_xy,
    };
    let event_delegate_ptr = Box::into_raw(Box::new(event_delegate)) as *mut c_void;

    let event_subview: id =
        unsafe { msg_send![event_proxy_view, initWithFrame:frame andDelegate:event_delegate_ptr] };
    if event_subview.is_null() {
        unsafe {
            Box::from_raw(event_delegate_ptr as *mut EventDelegate);
        } // drop EventDelegate

        return Err(SetupError::new_boxed(
            String::from("failed to intialize custom NSView").into(),
        ));
    }

    unsafe {
        let _: () = msg_send![parent, addSubview: event_proxy_view];
    }

    Ok(unsafe { StrongPtr::new(event_proxy_view) })
}

/// Stored within the `EventProxyView` class to support sending events back to Rust
/// from Objective-C callbacks.
struct EventDelegate {
    sender: Sender<WindowEvent>,
    size_xy: (i32, i32),
}

impl EventDelegate {
    /// Returns a mutable reference to an EventDelegate from an Objective-C callback.
    ///
    /// # Safety
    /// Caller must ensure no other thread is holding a reference to this object because
    /// [EventDelegate] is `!Sync`.
    // `clippy` has issues with this function signature, making the valid point that this could
    // create multiple mutable references to the `EventDelegate`. However, in practice macOS
    // blocks for the entire duration of each event callback, so this should be fine.
    //
    //#[allow(clippy::mut_from_ref)]
    unsafe fn from_field(obj: &Object) -> &EventDelegate {
        unsafe {
            let delegate_ptr: *mut c_void =
                *obj.get_ivar::<EventDelegateIvarType>(EVENT_DELEGATE_IVAR);
            &*(delegate_ptr as *mut EventDelegate)
        }
    }

    /// Convenience method to avoid `delegate.sender.send(...).unwrap()` boilerplate.
    fn send(&self, event: WindowEvent) {
        self.sender.send(event).unwrap();
    }
}

lazy_static::lazy_static! {
    /// Lazily initialized `NSView` subclass (`EventProxyView`) declaration that is capable of receiving
    /// window events through overloaded methods. Crucially, it holds an `EventDelegate` pointer so it
    /// can forward events back to the editor logic.
    static ref EVENT_PROXY_VIEW_CLASS: &'static Class = unsafe {
        let mut class = ClassDecl::new("EventProxyView", class!(NSView)).unwrap();

        class.add_method(sel!(init), class_methods::init as extern "C" fn(&mut Object, Sel) -> *mut Object);
        class.add_method(sel!(initWithFrame:), class_methods::init_with_frame as extern "C" fn(&mut Object, Sel, NSRect) -> *mut Object);
        class.add_method(sel!(initWithCoder:), class_methods::init_with_coder as extern "C" fn(&mut Object, Sel, id) -> *mut Object);
        class.add_method(sel!(initWithFrame:andDelegate:), class_methods::init_with_frame_and_delegate as extern "C" fn(&mut Object, Sel, NSRect, *mut c_void) -> *mut Object);
        class.add_method(sel!(dealloc), class_methods::dealloc as extern "C" fn(&mut Object, Sel));
        class.add_method(
            sel!(mouseDown:),
            class_methods::mouse_down as extern "C" fn(&mut Object, Sel, id),
        );
        class.add_method(sel!(mouseUp:), class_methods::mouse_up as extern "C" fn(&mut Object, Sel, id));
        class.add_method(
            sel!(rightMouseDown:),
            class_methods::right_mouse_down as extern "C" fn(&mut Object, Sel, id),
        );
        class.add_method(
            sel!(rightMouseUp:),
            class_methods::right_mouse_up as extern "C" fn(&mut Object, Sel, id),
        );
        class.add_method(
            sel!(otherMouseDown:),
            class_methods::other_mouse_down as extern "C" fn(&mut Object, Sel, id),
        );
        class.add_method(
            sel!(otherMouseUp:),
            class_methods::other_mouse_up as extern "C" fn(&mut Object, Sel, id),
        );
        class.add_method(
            sel!(mouseMoved:),
            class_methods::mouse_moved as extern "C" fn(&mut Object, Sel, id),
        );
        class.add_method(
            sel!(mouseDragged:),
            class_methods::mouse_dragged as extern "C" fn(&mut Object, Sel, id),
        );

        class.add_ivar::<EventDelegateIvarType>(EVENT_DELEGATE_IVAR);

        class.register()
    };
}

/// # Safety
/// - None of these functions should be called from Rust.
///
/// - Cocoa Documentation stipulates that NSView and descendants must
///   only be called from the main thread.
///   https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html
mod class_methods {
    use std::ffi::c_void;

    use cocoa::{base::id, foundation::NSRect};
    use objc::{
        class, msg_send,
        runtime::{Object, Sel},
        sel, sel_impl,
    };

    use crate::{MouseButton, WindowEvent};

    use super::{EventDelegate, EventDelegateIvarType, EVENT_DELEGATE_IVAR};

    pub extern "C" fn init(this: &mut Object, _sel: Sel) -> *mut Object {
        unsafe {
            let _: () = msg_send![this, release];
        }
        std::ptr::null_mut()
    }

    pub extern "C" fn init_with_frame(this: &mut Object, _sel: Sel, _frame: NSRect) -> *mut Object {
        unsafe {
            let _: () = msg_send![this, release];
        }
        std::ptr::null_mut()
    }

    pub extern "C" fn init_with_coder(this: &mut Object, _sel: Sel, _coder: id) -> *mut Object {
        unsafe {
            let _: () = msg_send![this, release];
        }
        std::ptr::null_mut()
    }

    pub extern "C" fn init_with_frame_and_delegate(
        this: &mut Object,
        _sel: Sel,
        frame: NSRect,
        delegate: *mut c_void,
    ) -> *mut Object {
        if delegate.is_null() {
            unsafe {
                let _: () = msg_send![this, release];
            }
            return std::ptr::null_mut();
        }

        let this: Option<&mut Object> =
            unsafe { msg_send![super(this, class!(NSView)), initWithFrame: frame] };

        match this {
            None => std::ptr::null_mut(),
            Some(this) => unsafe {
                this.set_ivar::<EventDelegateIvarType>(EVENT_DELEGATE_IVAR, delegate);
                this as *mut Object
            },
        }
    }

    pub extern "C" fn dealloc(this: &mut Object, _sel: Sel) {
        unsafe {
            let delegate_ptr: &mut *mut c_void =
                this.get_mut_ivar::<EventDelegateIvarType>(EVENT_DELEGATE_IVAR);

            if !(*delegate_ptr).is_null() {
                drop(Box::<EventDelegate>::from_raw(
                    *delegate_ptr as *mut EventDelegate,
                ));
                *delegate_ptr = std::ptr::null_mut(); // prevent double-free, use-after-free and similar shenanigans
            }
        }
    }

    // EventDelegate::from_field is safe to call because the methods are only ever called from the main thread

    pub extern "C" fn mouse_down(this: &mut Object, _sel: Sel, event: id) {
        let delegate = unsafe { send_cursor_movement_get_delegate(this, event) };

        delegate.send(WindowEvent::MouseClick(MouseButton::Left));
    }

    pub extern "C" fn mouse_up(this: &mut Object, _sel: Sel, event: id) {
        let delegate = unsafe { send_cursor_movement_get_delegate(this, event) };

        delegate.send(WindowEvent::MouseRelease(MouseButton::Left));
    }

    pub extern "C" fn right_mouse_down(this: &mut Object, _sel: Sel, event: id) {
        let delegate = unsafe { send_cursor_movement_get_delegate(this, event) };

        delegate.send(WindowEvent::MouseClick(MouseButton::Right));

        // TODO potentially call super https://developer.apple.com/documentation/appkit/nsview
    }

    pub extern "C" fn right_mouse_up(this: &mut Object, _sel: Sel, event: id) {
        let delegate = unsafe { send_cursor_movement_get_delegate(this, event) };

        delegate.send(WindowEvent::MouseRelease(MouseButton::Right));
    }

    pub extern "C" fn other_mouse_down(this: &mut Object, _sel: Sel, event: id) {
        let delegate = unsafe { send_cursor_movement_get_delegate(this, event) };

        delegate.send(WindowEvent::MouseClick(MouseButton::Middle));
    }

    pub extern "C" fn other_mouse_up(this: &mut Object, _sel: Sel, event: id) {
        let delegate = unsafe { send_cursor_movement_get_delegate(this, event) };

        delegate.send(WindowEvent::MouseRelease(MouseButton::Middle));
    }

    pub extern "C" fn mouse_moved(this: &mut Object, _sel: Sel, event: id) {
        unsafe { send_cursor_movement_get_delegate(this, event) };
    }

    pub extern "C" fn mouse_dragged(this: &mut Object, sel: Sel, event: id) {
        mouse_moved(this, sel, event)
    }

    unsafe fn send_cursor_movement_get_delegate(view: &mut Object, event: id) -> &EventDelegate {
        let window_location = unsafe { cocoa::appkit::NSEvent::locationInWindow(event) };
        let location = unsafe {
            cocoa::appkit::NSView::convertPoint_fromView_(
                view as id,
                window_location,
                std::ptr::null_mut(),
            )
        };

        let delegate = unsafe { EventDelegate::from_field(view) };

        delegate.send(WindowEvent::CursorMovement(
            (location.x / delegate.size_xy.0 as f64) as f32,
            1. - (location.y / delegate.size_xy.1 as f64) as f32,
        ));

        delegate
    }
}
