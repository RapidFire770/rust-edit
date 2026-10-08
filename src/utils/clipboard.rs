// arboard thin wrapper. Cargo enables `wayland-data-control`; arboard uses X11
// when that protocol is missing. Keep this value alive for the session: on
// Linux the copying process is what serves the clipboard data.
#![allow(dead_code)]

use arboard::Clipboard;

pub struct ClipboardWrapper {
    inner: Option<Clipboard>,
    #[cfg(test)]
    memory: Option<String>,
}

impl Default for ClipboardWrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardWrapper {
    pub fn new() -> Self {
        Self {
            inner: Clipboard::new().ok(),
            #[cfg(test)]
            memory: None,
        }
    }

    pub fn copy(&mut self, text: &str) -> bool {
        #[cfg(test)]
        {
            self.memory = Some(text.to_string());
        }
        if let Some(cb) = &mut self.inner {
            cb.set_text(text.to_string()).is_ok()
        } else {
            cfg!(test)
        }
    }

    pub fn paste(&mut self) -> Option<String> {
        if let Some(text) = self.inner.as_mut().and_then(|cb| cb.get_text().ok()) {
            return Some(text);
        }
        #[cfg(test)]
        {
            self.memory.clone()
        }
        #[cfg(not(test))]
        {
            None
        }
    }
}
