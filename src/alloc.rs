//! Счётчик аллокаций: глобальный аллокатор, считающий вызовы.
//!
//! Почему это в `lib`, а не в модуле: `#[global_allocator]` должен быть
//! **один на программу**. Если бы каждый модуль определял свой, они бы
//! конфликтовали при линковке. Один аллокатор в библиотеке — и доступен всем
//! 20 бинарникам. Это же и первая практика разделения «своё/чужая ответственность».
//!
//! Реальный проект: `dhat`, `jemallocator`, `mimalloc`, `snmalloc-rs`,
//! либо `tracking_allocator` — но идея всегда та же: перехватить `alloc`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub static ALLOCS: AtomicUsize = AtomicUsize::new(0);
pub static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
pub static DEALLOCS: AtomicUsize = AtomicUsize::new(0);
pub static DEALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
pub static LIVE: AtomicIsizeWrapper = AtomicIsizeWrapper::new(0);

/// Обёртка, чтобы не тянуть знаковый тип: `LIVE` умеет уходить в минус,
/// если где-то перепутают dealloc и alloc (баг, который надо увидеть).
pub struct AtomicIsizeWrapper(AtomicUsize);

impl AtomicIsizeWrapper {
    pub const fn new(v: isize) -> Self {
        Self(AtomicUsize::new(v as usize))
    }
    pub fn add(&self, v: isize) {
        self.0.fetch_add(v as usize, Ordering::Relaxed);
    }
    pub fn get(&self) -> isize {
        self.0.load(Ordering::Relaxed) as isize
    }
}

pub struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        LIVE.add(1);
        // SAFETY: контракт GlobalAlloc — вернуть невыровненный-но-валидный
        // блок нужного размера или null. System делает ровно это.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOCS.fetch_add(1, Ordering::Relaxed);
        DEALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        LIVE.add(-1);
        // SAFETY: ptr получен из System::alloc с тем же layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(new_size, Ordering::Relaxed);
        // Один realloc = один alloc + один dealloc.
        DEALLOCS.fetch_add(1, Ordering::Relaxed);
        DEALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: контракт realloc.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub allocs: usize,
    pub alloc_bytes: usize,
    pub deallocs: usize,
    pub live: isize,
}

pub fn snapshot() -> Snapshot {
    Snapshot {
        allocs: ALLOCS.load(Ordering::Relaxed),
        alloc_bytes: ALLOC_BYTES.load(Ordering::Relaxed),
        deallocs: DEALLOCS.load(Ordering::Relaxed),
        live: LIVE.get(),
    }
}

pub fn reset() {
    ALLOCS.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    DEALLOCS.store(0, Ordering::Relaxed);
    DEALLOC_BYTES.store(0, Ordering::Relaxed);
}

/// Печатает, сколько аллокаций произошло внутри замыкания `f`.
///
/// Нужен прогрев: первый вызов подтягивает лейауты, ленивые статики и кеш
/// аллокатора. Поэтому замыкание должно быть `FnMut` и вызываться дважды.
pub fn measure<T>(mut f: impl FnMut() -> T) -> (T, usize, usize) {
    let _ = f(); // прогрев
    reset();
    let value = f();
    let s = snapshot();
    (value, s.allocs, s.alloc_bytes)
}
