use super::Engine;

use std::any::Any;

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(super) fn panic_message(payload: Box<dyn Any + Send>) -> String {
        if let Some(s) = payload.downcast_ref::<&'static str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        }
    }
}
