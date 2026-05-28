// OPCUA vzljot for Rust
// SPDX-License-Identifier: MPL-2.0
// Copyright (C) 2026

//!OPC UA server for different flowmeters designed by developer "vzljot"
use std::{fs, io};
use std::path::PathBuf; //, sync::Arc}

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
    let str = std::net::TcpStream::connect(&device.device_ip_address)?;
    match device.device_type {
        DeviceType::LiteM => Ok("LiteM".to_string()),
        DeviceType::URSV5xx => Ok("URSV".to_string()),
    }
}
