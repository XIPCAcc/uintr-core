use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::task::Waker;
use std::sync::OnceLock;

#[derive(Clone)]
pub struct UintrToken {
    pub inner: Arc<Inner>,
    pub name: String,
}

pub struct Inner {
    pub seq: AtomicU32,
    pub consumed_seq: AtomicU32,
    pub waker: Mutex<Option<Waker>>,
}

static GLOBAL_TOKEN: OnceLock<UintrToken> = OnceLock::new();

impl UintrToken {
    pub fn new(name: &str) -> Self {
        Self {
            inner: Arc::new(Inner {
                seq: AtomicU32::new(0),
                consumed_seq: AtomicU32::new(0),
                waker: Mutex::new(None),
            }),
            name: name.to_string(),
        }
    }
    
    pub fn set_pending(&self) {
        self.inner.seq.fetch_add(1, Ordering::Release);
    }
    
    pub fn set_global_token(token: UintrToken) {
        let _ = GLOBAL_TOKEN.set(token);
    }
    
    pub fn get_global_token() -> Option<UintrToken> {
        GLOBAL_TOKEN.get().cloned()
    }
}

pub fn process_uintr_wakers(token: &UintrToken) -> u32 {
    let seq = token.inner.seq.load(Ordering::Acquire);
    let consumed = token.inner.consumed_seq.load(Ordering::Acquire);
    if seq == consumed {
        return 0;
    }

    let waker = token.inner.waker.lock().unwrap();
    if let Some(waker) = waker.as_ref() {
        waker.wake_by_ref();
        return 1;
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
