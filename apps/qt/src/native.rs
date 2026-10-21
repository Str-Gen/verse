//! Native Qt entry point. The reader QObject bridge remains in `bridge`.
#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("verse-qt/launch.h");
        fn run() -> i32;
    }
}

/// Run the Qt event loop.
///
/// # Safety
/// Call once on the process main thread before constructing any Qt objects.
/// Qt requires its GUI application and all window operations on this thread.
pub unsafe fn launch() -> i32 {
    ffi::run()
}
