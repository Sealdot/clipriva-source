#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().map(String::as_str) == Some("automation") {
        let socket_path = match std::env::var_os("HOME") {
            Some(home) => std::path::PathBuf::from(home)
                .join("Library/Application Support/com.clipriva.desktop/automation-v1.sock"),
            None => {
                eprintln!("clipriva automation: appUnavailable");
                std::process::exit(3);
            }
        };
        let code = clipriva_lib::automation::cli::run_cli(
            &arguments[1..],
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
            &socket_path,
        );
        std::process::exit(code as i32);
    }
    clipriva_lib::run();
}
