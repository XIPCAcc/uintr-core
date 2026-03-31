use std::sync::Arc;
use std::sync::Mutex;
use std::task::Waker;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct UintrToken {
    pub inner: Arc<Inner>,
    pub name: String,
}

pub struct Inner {
    pub pending: Mutex<bool>,
    pub waker: Mutex<Option<Waker>>,
}

static GLOBAL_TOKEN: OnceLock<UintrToken> = OnceLock::new();

impl UintrToken {
    pub fn new(name: &str) -> Self {
        Self {
            inner: Arc::new(Inner {
                pending: Mutex::new(false),
                waker: Mutex::new(None),
            }),
            name: name.to_string(),
        }
    }
    
    pub fn set_pending(&self) {
        *self.inner.pending.lock().unwrap() = true;
    }
    
    pub fn set_global_token(token: UintrToken) {
        let _ = GLOBAL_TOKEN.set(token);
    }
    
    pub fn get_global_token() -> Option<UintrToken> {
        GLOBAL_TOKEN.get().cloned()
    }
}

pub fn process_uintr_wakers(token: &UintrToken) -> u32 {
    let should_wake = {
        let pending = token.inner.pending.lock().unwrap();
        *pending
    };
    
    if should_wake {
        if let Some(waker) = token.inner.waker.lock().unwrap().take() {
            waker.wake();
            return 1;
        }
    }
    0
}

pub fn process_global_uintr_wakers() -> u32 {
    if let Some(token) = UintrToken::get_global_token() {
        process_uintr_wakers(&token)
    } else {
        0
    }
}
