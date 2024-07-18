use std::process::Command;
use std::error::Error;
use log::{error, info};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;
use crate::client::client;

fn setup_tun_interface() -> Result<(), Box<dyn Error>> {
    let output = Command::new("sudo")
        .arg("ip")
        .arg("link")
        .arg("set")
        .arg("dev")
        .arg("tun0")
        .arg("up")
        .output()?;

    if !output.status.success() {
        return Err(format!("Failed to bring up tun0: {:?}", output.stderr).into());
    } else {
        info!("Sucessfully brought up tun0");
    }

    let output = Command::new("sudo")
        .arg("ip")
        .arg("addr")
        .arg("add")
        .arg("10.8.0.1/24")
        .arg("dev")
        .arg("tun0")
        .output()?;

    if !output.status.success() {
        return Err(format!("Failed to assign IP to tun0: {:?}", output.stderr).into());
    } else {
        info!("Sucessfully assigned IP 10.8.0.1/24 to tun0");
    }

    Ok(())
}

async fn destroy_tun_interface() {
    let output = Command::new("sudo")
        .arg("ip")
        .arg("link")
        .arg("delete")
        .arg("tun0")
        .output()
        .expect("Failed to execute command to delete TUN interface");

    if !output.status.success() {
        eprintln!("Failed to delete TUN interface: {}", String::from_utf8_lossy(&output.stderr));
    }
}

pub fn server_mode() {
    let listener = TcpListener::bind("0.0.0.0:12345").unwrap();
    let clients: Arc<Mutex<HashMap<usize, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));

    let mut config = tun::Configuration::default();
    config.name("tun0");
    let tun_device = tun::create(&config).unwrap();

    // Setup the tun0 interface
    if let Err(e) = setup_tun_interface() {
        eprintln!("Failed to set up TUN interface: {}", e);
        return;
    }

    let shared_tun = Arc::new(Mutex::new(tun_device));

    info!("Server started on 0.0.0.0:12345");

    let tun_device_clone = shared_tun.clone();
    let clients_clone = clients.clone();

    thread::spawn(move || {
        let clients_guard = clients_clone.lock().unwrap();

        if let Some(client) = clients_guard.get(&0) { //TODO: Implement multi-client
            if let Ok(client_clone) = client.try_clone() {
                drop(clients_guard);  // Unlock the mutex early
                let mut locked_tun = tun_device_clone.lock().unwrap();
                client::read_from_tun_and_send_to_client(&mut *locked_tun, client_clone);
            } else {
                // Handle error while trying to clone the TcpStream
                println!("Failed to clone client TcpStream");
            }
        } else {
            // Handle the case where the client doesn't exist
            println!("No client with key 0 found");
        }
    });

    for (client_id, stream) in listener.incoming().enumerate() {
        match stream {
            Ok(stream) => {
                info!("New client connected with ID: {}", client_id);

                let tun_device_clone = shared_tun.clone();
                let clients_clone = clients.clone();

                // Insert the new client into the clients map
                {
                    let mut clients_guard = clients.lock().unwrap();
                    clients_guard.insert(client_id, stream.try_clone().unwrap());
                }

                // Spawn a thread to handle reading from TUN and sending to the client
                let tun_device_clone_for_thread = tun_device_clone.clone();
                let clients_clone_for_thread = clients_clone.clone();

                thread::spawn(move || {
                    let client_clone = {
                        let clients_guard = clients_clone_for_thread.lock().unwrap();
                        clients_guard.get(&client_id).unwrap().try_clone().unwrap()
                    };                   

                    let mut locked_tun = tun_device_clone_for_thread.lock().unwrap();
                    client::read_from_tun_and_send_to_client(&mut *locked_tun, client_clone);
                });

                //clients.lock().unwrap().insert(client_id, stream.try_clone().unwrap());
                let clients_arc = clients.clone();
                thread::spawn(move || client::handle_client(client_id, stream, clients_arc));
            }

            Err(e) => {
                error!("Connection failed: {}", e);
            }
        }
    }

    // Clean up the tun0 interface when done
    let _ = destroy_tun_interface();
}
