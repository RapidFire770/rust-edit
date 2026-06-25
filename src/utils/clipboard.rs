// arboard thin wrapper. Sprint 1.
#![allow(dead_code)]

use arboard::Clipboard;

#[derive(Default)]
pub struct ClipboardWrapper {
    inner: Option<Clipboard>,
}

impl ClipboardWrapper {
    pub fn new() -> Self {
        Self {
            inner: Clipboard::new().ok(),
        }
    }

    pub fn copy(&mut self, text: &str) -> bool {
        if let Some(cb) = &mut self.inner {
            cb.set_text(text.to_string()).is_ok()
        } else {
            false
        }
    }

    pub fn paste(&mut self) -> Option<String> {
        self.inner.as_mut().and_then(|cb| cb.get_text().ok())
    }
}
