// OPCUA vzljot for Rust
// SPDX-License-Identifier: MPL-2.0
// Copyright (C) 2026

//!OPC UA server for different flowmeters designed by developer "vzljot"
use std::io::{Read, Write};
use std::time::Duration;
use std::{fs, io};
use std::path::PathBuf; //, sync::Arc}
//use rand::prelude::*;

use rand::random;
use serde::{Deserialize, Serialize};
//use serde_json::Result;

struct Args {
    help: bool,
    config_path: PathBuf,
}

impl Default for Args {
    fn default() -> Self {

        let config_path = PathBuf::from("./config.json");

        Self {
            help: false,
             config_path,
        }
    }
}

impl Args {
    fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
        let mut args = pico_args::Arguments::from_env();

        let default = Args::default();
        let config_path: PathBuf = args
            .value_from_str(["-c", "--config"])
            .unwrap_or(default.config_path.clone());

        Ok(Args {
            help: args.contains(["-h", "--help"]),
              config_path,
        })
    }

    fn usage() {
        let args = Args::default();
        println!(
            r#"Demo Server
Usage:
  -h, --help                 Show help
  -c, --config [config-file] Path to a configuration file (default: {})"#,
              args.config_path.to_str().as_ref().unwrap()
        );
    }
}


#[derive(Deserialize, Serialize, Debug)]
enum DeviceType {
    LiteM,
    URSV5xx,
}

#[derive(Deserialize, Serialize, Debug)]
struct Device {
    device_ip_address: String,
    device_address: u8,
    device_type: DeviceType,
    device_name: String,
    connection_timeout: u64,
    read_timeout: u64,
}
fn main() {
    let args = Args::parse_args().unwrap();
    if args.help {
        Args::usage();
    } else {
        let devices: Vec<Device> = {
            let cf = fs::read_to_string(args.config_path).expect("Config file not found");
            serde_json::from_str(&cf).unwrap()
        };

        let test = request_device(&devices[0]).unwrap();
        println!("{:?}", test);
    }
}

fn request_device(device: &Device) -> Result<String, io::Error> {
    match device.device_type {
        DeviceType::LiteM => match request_lite_m(device) {
            Ok(answ) => Ok(answ.to_string()),
            Err(e) => Err(e),
        },
        DeviceType::URSV5xx => Ok("URSV".to_string()),
    }
}

fn request_lite_m(device_lite_m: &Device) -> Result<f32, io::Error> {
    let mut request: [u8; 12] = [0, 0, 0, 0, 0, 0x06, 0, 0x04, 0xC0, 0x08, 0, 0x02];
    request[6] = device_lite_m.device_address;
    let session_id: u16 = random::<u16>();
    request[0..2].copy_from_slice(&session_id.to_be_bytes());
 
    let mut str = std::net::TcpStream::connect_timeout(&device_lite_m.device_ip_address.parse::<core::net::SocketAddr>().unwrap(), 
        Duration::from_millis(device_lite_m.connection_timeout))?;

    str.write(&request)?;

    str.set_read_timeout(Some(Duration::from_millis(device_lite_m.read_timeout)))?;
 
    let mut answer: [u8; 56]= [0; 56];
    match str.read(&mut answer) {
        Ok(_) => Ok(f32::from_be_bytes([answer[9], answer[10], answer[11], answer[12]])),
        Err(e) => Err(e),
    }
}