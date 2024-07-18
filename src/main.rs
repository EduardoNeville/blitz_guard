mod client;
mod server;
mod encryption;

use clap::{App, Arg};
use env_logger::Builder;
use log::LevelFilter;

#[tokio::main]
async fn  main() {
    std::env::set_var("RUST_BACKTRACE", "1");

    // Initialize the logger with 'info' as the default level
    Builder::new()
        .filter(None, LevelFilter::Info)
        .init();

    let matches = App::new("BlitzGuard")
        .version("1.0")
        .author("Eduardo Neville")
        .about("A simple VPN tunnel in Rust")
        .arg(Arg::with_name("mode")
            .required(true)
            .index(1)
            .possible_values(&["server", "client"])
            .help("Runs the program in either server or client mode"))
        .arg(Arg::with_name("vpn-server")
            .long("vpn-server")
            .value_name("IP")
            .help("The IP address of the VPN server to connect to (client mode only)")
            .takes_value(true))
        .get_matches();

    let is_server_mode = matches.value_of("mode").unwrap() == "server";

    if is_server_mode {
        server::server::server_mode();
    } else {
        if let Some(vpn_server_ip) = matches.value_of("vpn-server") {
            let server_address = format!("{}:12345", vpn_server_ip);
            client::client::client_mode(server_address.as_str()).await;
        } else {
            eprintln!("Error: For client mode, you must provide the '--vpn-server' argument.");
        }
    }
}
