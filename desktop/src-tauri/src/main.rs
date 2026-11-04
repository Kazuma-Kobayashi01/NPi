#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--npi-python-host")) {
        if let Err(error) = omp_desktop_lib::python_runtime::run_host() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    // The Librarian as an MCP server for agents (librarian/mcp.rs): `--npi-mcp-librarian <project folder>`.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--npi-mcp-librarian")) {
        let Some(project) = std::env::args_os().nth(2) else {
            eprintln!("usage: --npi-mcp-librarian <project folder>");
            std::process::exit(2);
        };
        if let Err(error) = omp_desktop_lib::librarian::mcp::run(std::path::Path::new(&project)) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    // The Copilot as an MCP server (librarian/mcp.rs): `--npi-mcp-copilot <project folder>`.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--npi-mcp-copilot")) {
        let Some(project) = std::env::args_os().nth(2) else {
            eprintln!("usage: --npi-mcp-copilot <project folder>");
            std::process::exit(2);
        };
        if let Err(error) = omp_desktop_lib::librarian::mcp::run_copilot(std::path::Path::new(&project)) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    omp_desktop_lib::run();
}
