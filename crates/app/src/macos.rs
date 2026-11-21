//! macOS menu-bar (status bar) item: the next deadlines with a badge of what needs attention,
//! so reminders keep working with the window closed. Clicks are queued as events that the
//! GPUI app drains on its own thread; AppKit calls arrive on the main thread.
#![allow(unexpected_cfgs)] // objc 0.2 macros check a `cargo-clippy` feature

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, YES};
use objc::{class, msg_send, sel, sel_impl};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub enum Event {
    OpenWindow,
    OpenItem(i64),
    QuickAdd,
    About,
    Quit,
}

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());
static TARGET: AtomicUsize = AtomicUsize::new(0);
static ITEM: AtomicUsize = AtomicUsize::new(0);

fn push(e: Event) {
    EVENTS.lock().unwrap_or_else(|e| e.into_inner()).push(e);
}

pub fn take_events() -> Vec<Event> {
    std::mem::take(&mut *EVENTS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// An NSString the caller owns (release it).
unsafe fn ns_string(s: &str) -> *mut Object {
    unsafe {
        let obj: *mut Object = msg_send![class!(NSString), alloc];
        msg_send![obj, initWithBytes: s.as_ptr() length: s.len() encoding: 4usize]
    }
}

extern "C" fn menu_open(_: &Object, _: Sel, _s: *mut Object) {
    push(Event::OpenWindow);
}
extern "C" fn menu_add(_: &Object, _: Sel, _s: *mut Object) {
    push(Event::QuickAdd);
}
extern "C" fn menu_about(_: &Object, _: Sel, _s: *mut Object) {
    push(Event::About);
}
extern "C" fn menu_quit(_: &Object, _: Sel, _s: *mut Object) {
    push(Event::Quit);
}
extern "C" fn menu_item(_: &Object, _: Sel, sender: *mut Object) {
    // SAFETY: AppKit passes the NSMenuItem that was clicked.
    let tag: isize = unsafe { msg_send![sender, tag] };
    push(Event::OpenItem(tag as i64));
}

fn target() -> *mut Object {
    let existing = TARGET.load(Ordering::Acquire);
    if existing != 0 {
        return existing as *mut Object;
    }
    let cls = Class::get("VigiaBridge").unwrap_or_else(|| {
        let mut decl = ClassDecl::new("VigiaBridge", class!(NSObject)).expect("declare VigiaBridge");
        // SAFETY: method signatures match the selectors the menu items send.
        unsafe {
            decl.add_method(sel!(vigiaOpen:), menu_open as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_method(sel!(vigiaAdd:), menu_add as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_method(sel!(vigiaAbout:), menu_about as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_method(sel!(vigiaQuit:), menu_quit as extern "C" fn(&Object, Sel, *mut Object));
            decl.add_method(sel!(vigiaItem:), menu_item as extern "C" fn(&Object, Sel, *mut Object));
        }
        decl.register()
    });
    // SAFETY: plain alloc/init of our NSObject subclass; it lives for the whole process.
    let obj: *mut Object = unsafe {
        let o: *mut Object = msg_send![cls, alloc];
        msg_send![o, init]
    };
    TARGET.store(obj as usize, Ordering::Release);
    obj
}

/// What the status item shows; rebuilt whenever it changes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tray {
    /// Expired + due within 30 days; 0 hides the number.
    pub badge: usize,
    pub tooltip: String,
    pub header: String,
    /// (item id, label) of the next deadlines.
    pub items: Vec<(i64, String)>,
    pub empty: String,
    pub open: String,
    pub add: String,
    pub about: String,
    pub quit: String,
}

static LAST: Mutex<Option<Tray>> = Mutex::new(None);

/// Creates the status item once, then only updates it when `tray` differs from last time.
pub fn show(template_png: &[u8], tray: &Tray) {
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_ref() == Some(tray) && ITEM.load(Ordering::Acquire) != 0 {
            return;
        }
        *last = Some(tray.clone());
    }
    let t = target();
    // SAFETY: main-thread AppKit calls; objects are retained for the process lifetime.
    unsafe {
        let mut item = ITEM.load(Ordering::Acquire) as *mut Object;
        if item.is_null() {
            let bar: *mut Object = msg_send![class!(NSStatusBar), systemStatusBar];
            item = msg_send![bar, statusItemWithLength: -1.0f64];
            let _: *mut Object = msg_send![item, retain];
            ITEM.store(item as usize, Ordering::Release);
            let button: *mut Object = msg_send![item, button];
            let data: *mut Object = msg_send![class!(NSData), dataWithBytes: template_png.as_ptr() length: template_png.len()];
            let image: *mut Object = msg_send![class!(NSImage), alloc];
            let image: *mut Object = msg_send![image, initWithData: data];
            if !image.is_null() {
                let _: () = msg_send![image, setTemplate: YES];
                let _: () = msg_send![image, setSize: CGSize { width: 18.0, height: 18.0 }];
                let _: () = msg_send![button, setImage: image];
                let _: () = msg_send![button, setImagePosition: 2usize]; // NSImageLeft
            }
        }
        let button: *mut Object = msg_send![item, button];
        let tip = ns_string(&tray.tooltip);
        let _: () = msg_send![button, setToolTip: tip];
        let _: () = msg_send![tip, release];
        let title = ns_string(&if tray.badge == 0 { String::new() } else { format!(" {}", tray.badge) });
        let _: () = msg_send![button, setTitle: title];
        let _: () = msg_send![title, release];

        let menu: *mut Object = msg_send![class!(NSMenu), alloc];
        let menu: *mut Object = msg_send![menu, init];
        let empty = ns_string("");
        add_item(menu, &tray.header, None, empty, t, 0);
        if tray.items.is_empty() {
            add_item(menu, &tray.empty, None, empty, t, 0);
        }
        for (id, label) in &tray.items {
            add_item(menu, label, Some(sel!(vigiaItem:)), empty, t, *id as isize);
        }
        let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep];
        add_item(menu, &tray.open, Some(sel!(vigiaOpen:)), empty, t, 0);
        add_item(menu, &tray.add, Some(sel!(vigiaAdd:)), empty, t, 0);
        add_item(menu, &tray.about, Some(sel!(vigiaAbout:)), empty, t, 0);
        let sep: *mut Object = msg_send![class!(NSMenuItem), separatorItem];
        let _: () = msg_send![menu, addItem: sep];
        add_item(menu, &tray.quit, Some(sel!(vigiaQuit:)), empty, t, 0);
        let _: () = msg_send![item, setMenu: menu];
        let _: () = msg_send![menu, release];
        let _: () = msg_send![empty, release];
    }
}

/// Removes the status item (Settings → menu-bar item off).
pub fn hide() {
    let item = ITEM.swap(0, Ordering::AcqRel) as *mut Object;
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if item.is_null() {
        return;
    }
    // SAFETY: main-thread call removing the item we created and retained.
    unsafe {
        let bar: *mut Object = msg_send![class!(NSStatusBar), systemStatusBar];
        let _: () = msg_send![bar, removeStatusItem: item];
        let _: () = msg_send![item, release];
    }
}

unsafe fn add_item(menu: *mut Object, title: &str, action: Option<Sel>, key: *mut Object, target: *mut Object, tag: isize) {
    unsafe {
        let s = ns_string(title);
        let it: *mut Object = msg_send![class!(NSMenuItem), alloc];
        let it: *mut Object = match action {
            Some(a) => msg_send![it, initWithTitle: s action: a keyEquivalent: key],
            None => {
                let none: Sel = sel!(vigiaNothing);
                let it: *mut Object = msg_send![it, initWithTitle: s action: none keyEquivalent: key];
                // no target implements it: AppKit shows the row disabled (a header)
                it
            }
        };
        if action.is_some() {
            let _: () = msg_send![it, setTarget: target];
            let _: () = msg_send![it, setTag: tag];
        }
        let _: () = msg_send![menu, addItem: it];
        let _: () = msg_send![it, release];
        let _: () = msg_send![s, release];
    }
}

/// Bring the app forward (after reopening the window from the menu bar).
pub fn activate() {
    // SAFETY: main-thread AppKit call.
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, activateIgnoringOtherApps: YES];
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

// SAFETY: CGSize is two f64s, encoded as `{CGSize=dd}`.
unsafe impl objc::Encode for CGSize {
    fn encode() -> objc::Encoding {
        // SAFETY: the string is a valid type encoding for this struct.
        unsafe { objc::Encoding::from_str("{CGSize=dd}") }
    }
}
