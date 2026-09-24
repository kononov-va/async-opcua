// OPCUA metran900 for Rust
// SPDX-License-Identifier: MPL-2.0
// Copyright (C) 2026

//!OPC UA server for Metran 900
use std::{env, error::Error, fs::File, io::{self, Read, Write}, time::Duration};
use crc16::*;
use std::fmt;
//use fork::daemon;
use daemonize::Daemonize;

use opcua::{
    types::*,
    server::{ServerBuilder, diagnostics::NamespaceMetadata, address_space, 
            node_manager::memory::{simple_node_manager, SimpleNodeManager}},
};

struct DeviceAnswer{
    message_time: DateTime,
    message_items: Vec<f32>,
}

struct DeviceSettings{
    s_port: String,
    s_adderss: u8,
    s_timeout_s: u64, 
    s_retry: u8,
    s_verb: bool
}

fn main() {
    let mut verb = false;
    let mut port = String::from("/dev/ttyS3");
    let mut address: Vec<u8> = Vec::new();
    let mut timeout: u64 = 250;
    let mut retry: u8 = 2;
    let mut sysd_f: bool = false;

    let mut args = env::args().collect::<Vec<String>>().into_iter();
    loop {
        match args.next() {
            Some(arg) => {
                if arg == "-v" { //Выводить в лог ошибки связи
                    verb = true
                } else if arg == "-p" { //Уквзать порт в формате -p /devttySx (порт по умолчанию /dev/ttyS3)
                    port = args.next().expect("Wrong port format");
                } else if arg == "-a" {//Уквзать адреса приборов в формате -а 1,x,y и т.д.
                    let str_addr= args.next().expect("Wrong address format"); 
                    for ad in str_addr.split_terminator(",") {
                        address.push(ad.parse::<u8>().expect("Wrong address format"));
                    };
                } else if arg == "-t" {//Уквзать тфймаут в формате -t значение в миллисекундах
                    timeout = args.next().expect("Wrong timeout").parse().expect("Wrong timeout");
                } else if arg == "-r" {//Уквзать количество повторов при опросе в формате -r значение от 0 до 255
                    retry = args.next().expect("Wrong retry").parse().expect("Wrong retry");
                } else if arg == "-sysd" {//Не использовать старый механизм запуска фонового процесса
                    sysd_f = true;
                }
            },
            None => break,
        }
    }

    if sysd_f == false {
       let stdout = File::create("/tmp/daemon.out").unwrap();
       let stderr = File::create("/tmp/daemon.err").unwrap();

        let daemonize = Daemonize::new()
            .pid_file("/tmp/test.pid") // Every method except `new` and `start`
            .chown_pid_file(true) // is optional, see `Daemonize` documentation
            .working_directory("/tmp") // for default behaviour.
            .user("nobody")
            .group("daemon") // Group name
            .group(2) // or group id.
            .umask(0o777) // Set umask, `0o027` by default.
            .stdout(stdout) // Redirect stdout to `/tmp/daemon.out`.
            .stderr(stderr) // Redirect stderr to `/tmp/daemon.err`.
            .privileged_action(|| "Executed before drop privileges");

        match daemonize.start() {
            Ok(_) => println!("Success, daemonized."),
            Err(e) => eprintln!("Error, {}", e),
        }
    }

    start_ua_server(verb, port, address, timeout, retry);
}

#[tokio::main]
async fn start_ua_server(verb: bool, port: String, address: Vec<u8>, timeout_s: u64, retry: u8){
    env_logger::init();

    // Create an OPC UA server with sample configuration and default node set
    let mut server_builder = ServerBuilder::new().with_config_from("/etc/metran900/server.conf");
    for device_address in &address {
        server_builder = server_builder.with_node_manager(simple_node_manager(
                NamespaceMetadata {
                    namespace_uri: format!("urn:metran900_{}", *device_address).as_str().to_owned(),
                    ..Default::default()
                },
                device_address.to_string().as_str(),
            ))
    }
    let (server, handle) = server_builder
            .build()
            .unwrap();

    //let mut server = Server::new(ServerConfig::load(&PathBuf::from("/etc/metran900/server.conf")).unwrap());
    for device in address {
        let ns = handle.get_namespace_index(format!("urn:metran900_{}", device).as_str()).unwrap();

        let node_manager = handle
            .node_managers()
            .get_by_name::<SimpleNodeManager>(device.to_string().as_str())
            .unwrap();

        {
            let mut addr = node_manager.address_space().write();

            let folder_id = NodeId::new(ns, device.to_string());
                addr.add_folder(&folder_id, device.to_string(), 
                device.to_string(), &NodeId::objects_folder_id());

            let v_node_id = node_id::NodeId::new(ns, "items");
            let values = (0..12)
                .map(|_| 0f32.into())
                .collect::<Vec<Variant>>();
            let value_type = values.first().unwrap().type_id();
            let VariantTypeId::Scalar(s) = value_type else {
                panic!("Scalar values had array type");
            };

            address_space::VariableBuilder::new(&v_node_id, "items", "items")
                .data_type(DataTypeId::Float)
                .value_rank(1)
                .organized_by(&folder_id)
                .value((s, values))
                .insert(&mut *addr);

            let settings  = DeviceSettings {
                s_port: port.clone(),
                s_adderss: device,
                s_timeout_s: timeout_s,
                s_retry: retry,
                s_verb: verb,
            };
            node_manager.inner().add_read_callback(v_node_id.clone(), move |_, time_stamp, _| {
                match get_device_current_value(&settings, time_stamp) {
                    Ok(vl) => Ok(vl),
                    Err(_) => Ok(data_value::DataValue::new_now_status(0, opcua_types::StatusCode::BadCommunicationError)),
                }
            });
        }
    }

    server.run().await.unwrap();
}

fn get_device_current_value(settings: &DeviceSettings, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, Box<dyn Error>> {
    match get_device_message(settings.s_adderss, settings.s_port.clone(), Duration::from_millis(settings.s_timeout_s), settings.s_retry){
        Ok(answer) => Ok(format_answer(answer, time_stamp)),
        Err(e) => {
            if settings.s_verb == true {
                eprintln!("metran900: {} {}", settings.s_adderss, e);
            }
            Err(e)},
    }
}

fn format_answer(answer: DeviceAnswer, time_stamp: TimestampsToReturn) -> data_value::DataValue {
    let arr = answer.message_items.iter().map(|&x| x.into()).collect::<Vec<Variant>>();
    let type_id = arr[0].type_id();
    let VariantTypeId::Scalar(s) = type_id else {
        panic!("Scalar values had array type");
    };
    let mut res = data_value::DataValue::new_now(Array::new(s, arr).unwrap());
    res.set_timestamps(time_stamp, answer.message_time, answer.message_time);
    res
}
/*
fn add_variables(server: &mut Server, ns: u16, port: String, address: u8, verb: bool, timeout_s: u64, retry: u8) {

    let v1_node = NodeId::new(ns, format!("{}v1", address));
    let v2_node = NodeId::new(ns, format!("{}v2", address));
    let v3_node = NodeId::new(ns, format!("{}v3", address));
    let v4_node = NodeId::new(ns, format!("{}v4", address));
    let v5_node = NodeId::new(ns, format!("{}v5", address));
    let v6_node = NodeId::new(ns, format!("{}v6", address));
    let v7_node = NodeId::new(ns, format!("{}v7", address));
    let v8_node = NodeId::new(ns, format!("{}v8", address));
    let v9_node = NodeId::new(ns, format!("{}v9", address));
    let v10_node = NodeId::new(ns, format!("{}v10", address));
    let v11_node = NodeId::new(ns, format!("{}v11", address));
    let v12_node = NodeId::new(ns, format!("{}v12", address));

    let address_space = server.address_space();

    {
        let mut address_space = address_space.write();

        // Create a sample folder under objects folder
        let sample_folder_id = address_space
            .add_folder(address.to_string(), address.to_string(), &NodeId::objects_folder_id())
            .unwrap();

        // Add some variables to our sample folder. Values will be overwritten by the timer
        let _ = address_space.add_variables(
            vec![
                Variable::new(&v1_node, format!("{}v1", address), format!("{}v1", address), 0f64),
                Variable::new(&v2_node, format!("{}v2", address), format!("{}v2", address), 0f64),
                Variable::new(&v3_node, format!("{}v3", address), format!("{}v3", address), 0f64),
                Variable::new(&v4_node, format!("{}v4", address), format!("{}v4", address), 0f64),
                Variable::new(&v5_node, format!("{}v5", address), format!("{}v5", address), 0f64),
                Variable::new(&v6_node, format!("{}v6", address), format!("{}v6", address), 0f64),
                Variable::new(&v7_node, format!("{}v8", address), format!("{}v7", address), 0f64),
                Variable::new(&v8_node, format!("{}v8", address), format!("{}v8", address), 0f64),
                Variable::new(&v9_node, format!("{}v9", address), format!("{}v9", address), 0f64),
                Variable::new(&v10_node, format!("{}v10", address), format!("{}v10", address), 0f64),
                Variable::new(&v11_node, format!("{}v11", address), format!("{}v11", address), 0f64),
                Variable::new(&v12_node, format!("{}v12", address), format!("{}v12", address), 0f64),
            ],
            &sample_folder_id,
        );
    }
    {
        // Store a counter and a flag in a tuple
        server.add_polling_action(3000, move || {
            let mut address_space = address_space.write();

            match get_device_message(address, port.clone(), Duration::from_millis(timeout_s), retry){
                Ok(data) => {

                    let _ = address_space.find_variable_mut(v1_node.clone()).unwrap().set_value_direct(data.message_items[0],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v2_node.clone()).unwrap().set_value_direct(data.message_items[1],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v3_node.clone()).unwrap().set_value_direct(data.message_items[2],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v4_node.clone()).unwrap().set_value_direct(data.message_items[3],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v5_node.clone()).unwrap().set_value_direct(data.message_items[4],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v6_node.clone()).unwrap().set_value_direct(data.message_items[5],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v7_node.clone()).unwrap().set_value_direct(data.message_items[6],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v8_node.clone()).unwrap().set_value_direct(data.message_items[7],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v9_node.clone()).unwrap().set_value_direct(data.message_items[8],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v10_node.clone()).unwrap().set_value_direct(data.message_items[9],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v11_node.clone()).unwrap().set_value_direct(data.message_items[10],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);
                    let _ = address_space.find_variable_mut(v12_node.clone()).unwrap().set_value_direct(data.message_items[11],
                         opcua::types::StatusCode::Good, &data.message_time, &data.message_time);

                },
                Err(error) => {
                    if verb == true {
                        eprintln!("metran900: {address} {}", error);
                    }
                    let now = DateTime::now();

                    let _ = address_space.find_variable_mut(v1_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v2_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v3_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v4_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v5_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v6_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v7_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v8_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v9_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v10_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v11_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                    let _ = address_space.find_variable_mut(v12_node.clone()).unwrap().set_value_direct(0,
                         opcua::types::StatusCode::BadNoCommunication, &now, &now);
                },
            }
        } );
    };
}
*/
#[derive(Debug)]
struct ErrorWrongAnswer {}

impl Error for ErrorWrongAnswer {
    
}

impl fmt::Display for ErrorWrongAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid messge size")
    }
}

fn get_device_message(address: u8, serial_port: String, timeout: Duration, retry: u8) -> Result<DeviceAnswer, Box<dyn Error>>{
    let mut answer: [u8; 33] = [0; 33];

    let mut cnt: u8 = retry;
    while cnt > 0 {
        match ask_device(address, serial_port.clone(), &mut answer, timeout) {
            Ok(size) => {
                if size == 33{
                    match get_format_message(&answer) {
                        Ok(f_answer) => return Ok(f_answer),
                        Err(e) => return Err(Box::new(e)),
                    };
                }
                else {
                    return Err(Box::new(ErrorWrongAnswer {}));
                };
            },
            Err(ref e) if e.kind() == serialport::ErrorKind::Io(io::ErrorKind::TimedOut) => {cnt -= 1;},
            Err(e) => return Err(Box::new(e)),
        };
    };
    return Err(Box::new(TimeoutError {}));
}

fn ask_device(address: u8, serial_port: String, buf: &mut [u8], timeout: Duration) -> Result<usize, serialport::Error> {
    let mut message: [u8; 7] = [0, 0x55, 0x4d, 0xff, 0xff, 0, 0];
    message[0] = address;
 
    let mut port = match serialport::new(serial_port, 9600).timeout(timeout).open() {
        Ok(p) => p,
        Err(e) => return Err(e),
    };

    match port.write(&message) {
        Ok(_) => {},
        Err(e) => return Err(Into::into(e)), 
    }

    match port.read_exact(buf) {
        Ok(_) => Ok(33),
        Err(e) => return Err(Into::into(e)),
    }
}

#[derive(Debug)]
struct CRCError {}

impl Error for CRCError {}

impl fmt::Display for CRCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CRC error")
    }
}

fn get_format_message(buf: &[u8]) -> Result<DeviceAnswer, CRCError>{
    if u16::from_le_bytes([buf[31], buf[32]]) == State::<XMODEM>::calculate(&buf[0..31]) {
        let formated_answer = DeviceAnswer{
            message_time: DateTime::now(),
            message_items: buf[5..29].chunks(2)
                .map(|x| f32::from(i16::from_le_bytes([x[0], x[1]]))/32.0).collect(),
        };
        Ok(formated_answer)
    }
    else {
        Err(CRCError {  })
    }
}

#[derive(Debug)]
struct TimeoutError {}

impl Error for TimeoutError {}

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TimeoutError error")
    }
}
