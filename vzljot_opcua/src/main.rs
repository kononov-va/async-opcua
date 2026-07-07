// OPCUA vzljot for Rust
// SPDX-License-Identifier: MPL-2.0
// Copyright (C) 2026

//!OPC UA server for different flowmeters designed by developer "vzljot"
use std::{fs, io};
use std::path::PathBuf;

use opcua::types;
use opcua_types::NodeId;
use serde::{Deserialize, Serialize};
use opcua::{
    types::{data_value, node_id, TimestampsToReturn},
    server::{ServerConfig, ServerBuilder, diagnostics::NamespaceMetadata, address_space},
};

use crate::vzljot_node_manager::VzljotNodeManager;

mod lite_m;
mod ursv5xx;
mod vzljot_node_manager;

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


#[derive(Deserialize, Serialize, Debug, Clone)]
enum DeviceType {
    LiteM,
    URSV5xx,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Device {
    device_ip_address: String,
    device_address: u8,
    device_type: DeviceType,
    device_name: String,
    connection_timeout: u64,
    read_timeout: u64,
    history_read_await: u64,
}

#[derive(Deserialize, Serialize, Debug)]
struct VzljotServerConfig {
    server_config: ServerConfig,
    devices: Vec<Device>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse_args().unwrap();
    if args.help {
        Args::usage();
    } else {
        let config: VzljotServerConfig = {
            let cf = fs::read_to_string(args.config_path).expect("Config file not found");
            serde_json::from_str(&cf).unwrap()
        };

        let mut server_builder = ServerBuilder::new().with_config(config.server_config.clone());
        for device in &config.devices{
            server_builder = server_builder.with_node_manager(vzljot_node_manager::vzljot_node_manager(
                 NamespaceMetadata {
                    namespace_uri: format!("urn:VzljotServer_{}", device.device_name).as_str().to_owned(),
                    ..Default::default()
            }, device.device_name.as_str(), device.clone()));
        }
        //println!("{:?}", config.server_config.endpoints);
        let (server, handle) = server_builder
            .build()
            .unwrap();

        for device in &config.devices {

            let ns = handle.get_namespace_index(format!("urn:VzljotServer_{}", device.device_name).as_str()).unwrap();

            let node_manager = handle
                .node_managers()
                .get_by_name::<VzljotNodeManager>(device.device_name.as_str())
                .unwrap();

            {
                let mut addr = node_manager.address_space().write();

                let folder_id = NodeId::new(ns, device.device_name.clone());
                addr.add_folder(&folder_id, device.device_name.clone(), 
                    device.device_name.clone(), &NodeId::objects_folder_id());

                let v_node_id = node_id::NodeId::new(ns, "V");

                address_space::VariableBuilder::new(&v_node_id, "V", "V")
                    .history_readable()
                    .historizing(true)
                    .organized_by(&folder_id)
                    .data_type(types::generated::node_ids::DataTypeId::Double)
                    .value(device.device_address)
                    .insert(&mut *addr);

                let dv_v = node_manager.inner().get_device();
                node_manager.inner().add_read_callback(v_node_id.clone(), move |_, time_stamp, _| {
                    match get_device_current_volume(&dv_v, time_stamp) {
                        Ok(vl) => Ok(vl),
                        Err(_) => Ok(data_value::DataValue::new_now_status(0, opcua_types::StatusCode::BadCommunicationError)),
                    }
                });

                let q_node_id = node_id::NodeId::new(ns, "Q");

                address_space::VariableBuilder::new(&q_node_id, "Q", "Q")
                    .organized_by(&folder_id)
                    .data_type(types::generated::node_ids::DataTypeId::Float)
                    .value(0)
                    .insert(&mut *addr);              

                let dv = node_manager.inner().get_device();
                node_manager.inner().add_read_callback(q_node_id.clone(), move |_, time_stamp, _| {
                    match get_device_current_value(&dv, time_stamp) {
                        Ok(vl) => Ok(vl),
                        Err(_) => Ok(data_value::DataValue::new_now_status(0, opcua_types::StatusCode::BadCommunicationError)),
                    }
                });
            }
        }

        server.run().await.unwrap();
    }
}

fn get_device_current_value(device: &Device, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, io::Error> {
    match device.device_type {
        DeviceType::LiteM => {
            match crate::lite_m::request_lite_m(device, time_stamp) {
                Ok(answ) => Ok(answ),
                Err(e) => Err(e),
            }       
        },
        DeviceType::URSV5xx => {
            match crate::ursv5xx::request_ursv5xx(device, time_stamp) {
                Ok(answ) => Ok(answ),
                Err(e) => Err(e),
            }
        },
    }
}

pub(crate) fn get_device_current_volume(device: &Device, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, io::Error> {
    match device.device_type {
        DeviceType::LiteM => {
             match crate::lite_m::request_lite_m_volume(device, time_stamp) {
                Ok(answ) => Ok(answ),
                Err(e) => Err(e),
            }       
        },
        DeviceType::URSV5xx => {
             match crate::ursv5xx::request_ursv5xx_volume(device, time_stamp) {
                Ok(answ) => Ok(answ),
                Err(e) => Err(e),
            }       
        },
    }
}

pub(crate) fn request_period(device: &Device, start: opcua::types::data_types::UtcTime,
    end: opcua::types::data_types::UtcTime, time_stamp: TimestampsToReturn, 
    bounds: bool, num_values_per_node: u32) -> (Option<Vec<opcua::types::data_value::DataValue>>, opcua_types::StatusCode) {
    match device.device_type {
        DeviceType::LiteM => {
            crate::lite_m::request_lite_m_arhive_period(device, start, end, time_stamp, bounds, num_values_per_node)       
        },
        DeviceType::URSV5xx => {
            crate::ursv5xx::request_ursv5xx_arhive_period(device, start, end, time_stamp, bounds, num_values_per_node)       
        },
    }
}
