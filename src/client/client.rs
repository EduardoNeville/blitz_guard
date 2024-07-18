use std::process::Command;
use std::io::{Read, Write};
use std::net::{TcpStream, Shutdown};
use log::{error, info};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use serde_derive::Serialize;
use serde_derive::Deserialize;
use crate::encryption::encryption;
use tun::platform::Device;

#[derive(Serialize, Deserialize)]
struct VpnPacket {
    data: Vec<u8>,
}

const TUN_INTERFACE_NAME: &str = "tun1";

fn set_client_ip_and_route() {
    let ip_output = Command::new("ip")
        .arg("addr")
        .arg("add")
        .arg("10.8.0.2/24")
        .arg("dev")
        .arg("tun0")
        .output()
        .expect("Failed to execute IP command");

    if !ip_output.status.success() {
        eprintln!("Failed to set IP: {}", String::from_utf8_lossy(&ip_output.stderr));
        return;
    } else {
        info!("New ip addr added at 10.8.0.2/24");
    }

    let link_output = Command::new("ip")
        .arg("link")
        .arg("set")
        .arg("up")
        .arg("dev")
        .arg("tun0")
        .output()
        .expect("Failed to execute IP LINK command");

    if !link_output.status.success() {
        eprintln!("Failed to set link up: {}", String::from_utf8_lossy(&link_output.stderr));
        return;
    } else {
        info!("Ip set sucessfully to device tun0");
    }

    let route_output = Command::new("ip")
        .arg("route")
        .arg("add")
        .arg("0.0.0.0/0")
        .arg("via")
        .arg("10.8.0.1")
        .arg("dev")
        .arg("tun0")
        .output()
        .expect("Failed to execute IP ROUTE command");

    if !route_output.status.success() {
        eprintln!("Failed to set route: {}", String::from_utf8_lossy(&route_output.stderr));
    } else {
        info!("Ip route added sucessfully from 0.0.0.0 via 10.8.0.1");
    }
}

pub async fn client_mode(vpn_server_ip: &str) {
    // Basic client mode for demonstration
    let mut stream = TcpStream::connect(vpn_server_ip).unwrap();

    // Clone the stream we can use it both inside and outside the async block
    let mut stream_clone = stream.try_clone().unwrap();

    let mut config = tun::Configuration::default();

    // Different to the server device!!
    config.name(TUN_INTERFACE_NAME);

    // Extra linux config
    #[cfg(target_os = "linux")]
	config.platform(|config| {
		config.packet_information(true);
	});

    let mut tun_device = tun::platform::linux::create(&config).unwrap();
    //let mut tun_device = tun::create(&config).unwrap();

    // Set the client's IP and routing
    set_client_ip_and_route();

    info!("Connected to the server {}", vpn_server_ip);

    let mut buffer = [0; 1024];
    loop {
        match stream.read(&mut buffer) {
            Ok(n) => {
                info!("{} Received from the server", n);
                read_from_client_and_write_to_tun(&mut stream_clone, &mut tun_device).await;
            }
            Err(_) => {
                break;
            }
        }
    }
}


pub fn handle_client(client_id: usize, mut stream: TcpStream, clients: Arc<Mutex<HashMap<usize, TcpStream>>>) {
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                info!("Client {} disconnected", client_id);
                break;
            }
            Ok(n) => {
                let data = &buffer[0..n];

                info!("Server: data received from the client: {:?}", data);

                let mut clients_guard = clients.lock().unwrap();

                for (id, client_stream) in clients_guard.iter_mut() {
                    if *id != client_id {
                        let _ = client_stream.write(data);
                    }
                }
            }
            Err(e) => {
                error!("Error reading from client {}: {}", client_id, e);
                break;
            }
        }
    }

    clients.lock().unwrap().remove(&client_id);
    let _ = stream.shutdown(Shutdown::Both);
}

pub fn read_from_tun_and_send_to_client<T: tun::Device>(tun: &mut T, mut client: TcpStream) {
    let mut buffer = [0u8; 1500];

    loop {
        match tun.read(&mut buffer) {
            Ok(n) => {
                match encryption::encrypt(&buffer[..n]) {
                    Ok(encrypted_data) => {
                        // Handle sending the encrypted data to the client
                        info!("Received {} bytes from TUN device.", n);

                        let vpn_packet = VpnPacket { data: encrypted_data };
                        // Serialize and send to client
                        let serialized_data = bincode::serialize(&vpn_packet).unwrap();

                        client.write_all(&serialized_data).unwrap();
                        info!("Forwarded {} bytes to destination.", n);

                    },
                    Err(err_msg) => {
                        // Handle the encryption error
                        error!("Encryption error: {}", err_msg);
                    }
                }
            },
            Err(e) => {
                // Handle the TUN reading error
                error!("TUN read error: {}", e);
            }
        }
    }
}

pub async fn read_from_client_and_write_to_tun(client: &mut TcpStream, tun: &mut Device) {
    let mut buffer = [0u8; 1500];
    loop {
        match client.read(&mut buffer) {
            Ok(n) => {
                let vpn_packet: VpnPacket = bincode::deserialize(&buffer[..n]).unwrap();
                let decrypted_data = encryption::decrypt(&vpn_packet.data);

                info!("Writing data to tun0");
                let _ = tun.write(&decrypted_data);
            }
            Err(e) => {
                error!("Error reading from client: {}", e);
                continue;
            }
        };
    }
}
