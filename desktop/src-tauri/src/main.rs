#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--npi-python-host")) {
        if let Err(error) = omp_desktop_lib::python_runtime::run_host() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    omp_desktop_lib::run();
}
