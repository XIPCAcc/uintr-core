use std::io::{self, Write};
use std::os::unix::net::UnixStream;
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

pub struct GlobalUintr {
    pub token: UintrToken,
    sender: UnixStream,
    receiver: UnixStream,
}

static GLOBAL_UINTR: OnceLock<GlobalUintr> = OnceLock::new();

impl GlobalUintr {
    fn new(token: UintrToken) -> io::Result<Self> {
        let (receiver, sender) = UnixStream::pair()?;
        receiver.set_nonblocking(true)?;
        sender.set_nonblocking(true)?;

        Ok(Self {
            token,
            sender,
            receiver,
        })
    }

    pub fn notify(&self) -> io::Result<()> {
        self.token.set_pending();

        // 使用 try_lock 检测 waker，避免死锁
        if let Ok(waker) = self.token.inner.waker.try_lock() {
            if waker.is_some() {
                let mut sender = &self.sender;
                sender.write_all(&[1])?;
            }
        }
        Ok(())
    }

    pub fn try_clone_receiver(&self) -> io::Result<UnixStream> {
        self.receiver.try_clone()
    }
}

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
        let _ = init_global_uintr(token);
    }
    
    pub fn get_global_token() -> Option<UintrToken> {
        global_uintr().map(|global| global.token.clone())
    }
}

pub fn init_global_uintr(token: UintrToken) -> io::Result<&'static GlobalUintr> {
    if let Some(global) = GLOBAL_UINTR.get() {
        return Ok(global);
    }

    let global = GlobalUintr::new(token)?;
    match GLOBAL_UINTR.set(global) {
        Ok(()) => Ok(GLOBAL_UINTR.get().expect("global uintr just initialized")),
        Err(_) => Ok(GLOBAL_UINTR.get().expect("global uintr should exist")),
    }
}

pub fn global_uintr() -> Option<&'static GlobalUintr> {
    GLOBAL_UINTR.get()
}

pub fn notify_global_uintr() -> io::Result<()> {
    match global_uintr() {
        Some(global) => global.notify(),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "global uintr is not initialized",
        )),
    }
}

pub fn try_clone_global_receiver() -> io::Result<UnixStream> {
    match global_uintr() {
        Some(global) => global.try_clone_receiver(),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "global uintr is not initialized",
        )),
    }
}

pub fn process_uintr_wakers(token: &UintrToken) -> u32 {
    let seq = token.inner.seq.load(Ordering::Acquire);
    let consumed = token.inner.consumed_seq.load(Ordering::Acquire);
    if seq == consumed {
        return 0;
    }

    let mut waker_guard = token.inner.waker.lock().unwrap();
    if let Some(waker) = waker_guard.take() {
        waker.wake();
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
