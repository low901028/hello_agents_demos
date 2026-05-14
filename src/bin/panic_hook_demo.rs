use std::backtrace::Backtrace;
use std::panic::PanicHookInfo;

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info: &PanicHookInfo<'_>| {
        let location = panic_info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown".to_string());

        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|msg| (*msg).to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_string());

        eprintln!("GOOSED_BOOT: panic at {location}: {payload}");
        eprintln!("GOOSED_BOOT: backtrace:\n{}", Backtrace::force_capture());

        default_hook(panic_info);
    }));
}

fn main() {
    install_panic_hook();
    println!("====panic hook .......");
    panic!("Oops!");
    println!("====panic hook end......");
}