// OPCUA vzljot for Rust
// SPDX-License-Identifier: MPL-2.0
// Copyright (C) 2026

//!OPC UA server for different flowmeters designed by developer "vzljot"
use core::fmt;
use std::io::{Read, Write};
use std::time::Duration;
use std::{fs, io};
use std::path::PathBuf; //, sync::Arc}
use chrono::{Local, prelude::*};

//use opcua::types::ChassisIdSubtype::Local as opcua_Local;
use rand::random;
use serde::{Deserialize, Serialize};
use opcua::types::{self, DateTime, data_value};
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
        DeviceType::LiteM => {
            let st; 
            match request_lite_m(device) {
                Ok(answ) => {st = answ.to_string(); 
                    let arc = match request_lite_m_arch(device, chrono::Local.with_ymd_and_hms(2026, 6, 5, 14, 0, 0).unwrap()) {
                        Ok(answ) => Ok(answ),
                        Err(e) => Err(e)
                    };
                    Ok(format!("{} {:?}", st, arc))},
                Err(e) => Err(e),
            }       
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

fn request_lite_m_arch(device_lite_m: &Device, request_time: chrono::DateTime<chrono::Local>) -> Result<data_value::DataValue, io::Error> {
    let mut request: [u8; 19] = [0, 0, 0, 0, 0, 0x0D, 0, 0x41, 0, 0x01, 0, 0x01, 0x01, 0, 0, 0, 0, 0, 0];
    request[6] = device_lite_m.device_address;
    let session_id: u16 = random::<u16>();
    request[0..2].copy_from_slice(&session_id.to_be_bytes());
    //request[13] = u8::from(request_time.second().to_be_bytes()[3]);
    //request[14] = u8::from(request_time.minute().to_be_bytes()[3]);
    request[15] = u8::from(request_time.hour().to_be_bytes()[3]);
    request[16] = u8::from(request_time.day().to_be_bytes()[3]);
    request[17] = u8::from(request_time.month().to_be_bytes()[3]);
    request[18] = u8::from((request_time.year() - 2000).to_be_bytes()[3]);

    let mut str = std::net::TcpStream::connect_timeout(&device_lite_m.device_ip_address.parse::<core::net::SocketAddr>().unwrap(), 
        Duration::from_millis(device_lite_m.connection_timeout))?;

    str.write(&request)?;

    str.set_read_timeout(Some(Duration::from_millis(device_lite_m.read_timeout)))?;
 
    let mut answer: [u8; 40]= [0; 40];
    match str.read(&mut answer) {
        Ok(_) => {
              Ok(data_value::DataValue::new_at( get_lite_m_volume(&answer[13..29]), 
                DateTime::from(chrono::DateTime::from_timestamp_secs(i64::from(u32::from_be_bytes([answer[9], answer[10], answer[11], answer[12]]))).unwrap())))
        }
        Err(e) => Err(e),
    }    
}

fn get_lite_m_volume(buffer: &[u8]) -> f64
{
    let positive_i = i32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
    let positive_f = f32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
    let negative_i = i32::from_be_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);
    let negative_f = f32::from_be_bytes([buffer[12], buffer[13], buffer[14], buffer[15]]);

    f64::from(positive_i) + f64::from(positive_f) - f64::from(negative_i) - f64::from(negative_f)
}