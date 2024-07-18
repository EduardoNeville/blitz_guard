# Blitz Guard

## How to it works?

Commands to run:

First you will run the build command to create the executable
```bash
cargo build --all-features
```

You then have to start the server with the following command
```bash
sudo cargo run server
```

To run a client you will have to link a vpn server ip. Atm it's hardcoded to
10.8.0.1 this should be changed soon

```bash
sudo cargo run client --vpn-server 10.8.0.1
```






