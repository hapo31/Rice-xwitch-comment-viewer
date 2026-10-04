//! Test-only System wrapper. The budget test runs in an isolated child process;
//! this measures Rust-owned live heap, not RSS, native libraries or WebView/GPU.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
struct CountedSystem;
#[global_allocator]
static ALLOCATOR: CountedSystem = CountedSystem;

fn record(size: usize) {
    let live = LIVE.fetch_add(size, Relaxed) + size;
    PEAK.fetch_max(live, Relaxed);
}

// SAFETY: Every allocation, zeroed allocation, reallocation and deallocation
// delegates unchanged pointers/layouts to System. Accounting only uses atomics,
// never allocates, and never unwinds. Failed allocations change no counters.
unsafe impl GlobalAlloc for CountedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe {
            System.dealloc(pointer, layout);
        }
        LIVE.fetch_sub(layout.size(), Relaxed);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            // Count the temporary coexistence of old/new buffers conservatively.
            record(size);
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        result
    }
}

pub(crate) fn start() -> usize {
    let baseline = LIVE.load(Relaxed);
    PEAK.store(baseline, Relaxed);
    baseline
}
pub(crate) fn peak_delta(baseline: usize) -> usize {
    PEAK.load(Relaxed).saturating_sub(baseline)
}
