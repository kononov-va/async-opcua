// OPCUA vzljot for Rust
// SPDX-License-Identifier: MPL-2.0
// Copyright (C) 2026

//!OPC UA server for different flowmeters designed by developer "vzljot"
use std::io::{Read, Write};
use std::net::TcpStream;
use std::ops::{Add, AddAssign, Sub};
use std::thread::sleep;
use std::time::Duration;
use std::io;
use chrono::{prelude::*};

use rand::random;
use opcua::types::{data_value, TimestampsToReturn};
//use tokio::time::error::Error;

use crate::Device;

pub(crate) fn request_ursv5xx(device: &Device, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, io::Error> {
    let mut request: [u8; 12] = [0, 0, 0, 0, 0, 0x06, 0, 0x04, 0x81, 0x5A, 0, 0x02];
    request[6] = device.device_address;
    let session_id: u16 = random::<u16>();
    request[0..2].copy_from_slice(&session_id.to_be_bytes());
 
    let mut str = std::net::TcpStream::connect_timeout(&device.device_ip_address.parse::<core::net::SocketAddr>().unwrap(), 
        Duration::from_millis(device.connection_timeout))?;

    str.write(&request)?;

    str.set_read_timeout(Some(Duration::from_millis(device.read_timeout)))?;
 
    let mut answer: [u8; 56]= [0; 56];
    match str.read(&mut answer) {
        Ok(_) => {
            let id = u16::from_be_bytes([answer[0], answer[1]]);
            if id == session_id{
                let mut dv = data_value::DataValue::new_now(f32::from_be_bytes([answer[9], answer[10], answer[11], answer[12]])*0.06);
                dv.set_timestamps(time_stamp, 
                    opcua_types::DateTime::from(Utc::now()), 
                    opcua_types::DateTime::from(Utc::now()));
                Ok(dv)
            }
            else {
                Err(io::Error::new(io::ErrorKind::Other, "Bad session id"))
            }
        },
        Err(e) => Err(e),
    }
}

pub(crate) fn request_ursv5xx_volume(device: &Device, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, io::Error> {
    let mut request: [u8; 12] = [0, 0, 0, 0, 0, 0x06, 0, 0x04, 0x80, 0x22, 0, 0x08];
    request[6] = device.device_address;
    let session_id: u16 = random::<u16>();
    request[0..2].copy_from_slice(&session_id.to_be_bytes());
 
    let mut str = std::net::TcpStream::connect_timeout(&device.device_ip_address.parse::<core::net::SocketAddr>().unwrap(), 
        Duration::from_millis(device.connection_timeout))?;

    str.write(&request)?;

    str.set_read_timeout(Some(Duration::from_millis(device.read_timeout)))?;
 
    let mut answer: [u8; 56]= [0; 56];
    match str.read(&mut answer) {
        Ok(_) => {
                let id = u16::from_be_bytes([answer[0], answer[1]]);
                if id == session_id{
                let mut dv = data_value::DataValue::new_now(get_ursv5xx_volume( &answer[9..25]));
                dv.set_timestamps(time_stamp, 
                    opcua_types::DateTime::from(Utc::now()), 
                    opcua_types::DateTime::from(Utc::now()));
                Ok(dv)
            }
            else {
                Err(io::Error::new(io::ErrorKind::Other, "Bad session id"))
            }
        },
        Err(e) => Err(e),
    }
}

fn request_ursv5xx_arch_time(device_lite_m: &Device) -> Result<DateTime<Local>, io::Error> {
    let mut request: [u8; 12] = [0, 0, 0, 0, 0, 0x06, 0, 0x04, 0x80, 0x06, 0, 0x02];
    request[6] = device_lite_m.device_address;
    let session_id: u16 = random::<u16>();
    request[0..2].copy_from_slice(&session_id.to_be_bytes());
 
    let mut str = std::net::TcpStream::connect_timeout(&device_lite_m.device_ip_address.parse::<core::net::SocketAddr>().unwrap(), 
        Duration::from_millis(device_lite_m.connection_timeout))?;

    str.write(&request)?;

    str.set_read_timeout(Some(Duration::from_millis(device_lite_m.read_timeout)))?;
 
    let mut answer: [u8; 20]= [0; 20];
    match str.read(&mut answer) {
        Ok(_) => {
            let time_source = u32::from_be_bytes([answer[9], answer[10], answer[11], answer[12]]);
            let t = DateTime::<Local>::from(DateTime::from_timestamp_secs(i64::from(time_source)).unwrap());//.with_timezone(&Local);
            Ok(t)

        },
        Err(e) => Err(e),
    }
}

fn request_ursv5xx_end_point(device: &Device, time_stamp: TimestampsToReturn) -> (Option<Vec<opcua::types::data_value::DataValue>>, opcua_types::StatusCode) {
    let mut result: Vec<opcua::types::data_value::DataValue> = Vec::new();
    let mut status_result = opcua_types::StatusCode::BadCommunicationError;

    let time = match request_ursv5xx_arch_time(device) {
        Ok(i) => i,
        Err(_) => {return (None, status_result);}
    };

    let value = match request_ursv5xx_at_time(device, time, time_stamp) {
        Ok(mut val) => {val.set_timestamps(time_stamp, val.source_timestamp.unwrap(), 
                val.server_timestamp.unwrap());
            status_result = opcua_types::StatusCode::Good;
            val},
        Err(_) => {return (None, status_result);}
    };
    result.push(value);     
    (Some(result), status_result)
}

fn request_ursv5xx_start_point(device: &Device, time_stamp: TimestampsToReturn) -> (Option<Vec<opcua::types::data_value::DataValue>>, opcua_types::StatusCode) {
    let mut result: Vec<opcua::types::data_value::DataValue> = Vec::new();
    let mut status_result = opcua_types::StatusCode::BadCommunicationError;

    let mut time: DateTime<Local> = match request_ursv5xx_arch_time(device) {
        Ok(i) => i,
        Err(_) => {return (None, status_result);}
    };
    time.add_assign(Duration::from_hours(1));

    let value = match request_ursv5xx_at_time(device, time, time_stamp) {
        Ok(mut val) => {val.set_timestamps(time_stamp, val.source_timestamp.unwrap(), 
                val.server_timestamp.unwrap());
            status_result = opcua_types::StatusCode::Good;
            val},
        Err(_) => {return (None, status_result);}
    };

    if value.status.unwrap() != opcua_types::StatusCode::Good {
        let value = match request_ursv5xx_by_index(device, 0, time_stamp) {
            Ok(mut val) => {val.set_timestamps(time_stamp, val.source_timestamp.unwrap(), 
                    val.server_timestamp.unwrap());
                status_result = opcua_types::StatusCode::Good;
                val},
            Err(_) => {return (None, status_result);}
        };
        result.push(value);     
        return (Some(result), status_result);  
    }

    result.push(value);     
    (Some(result), status_result)
}

pub(crate) fn request_ursv5xx_arhive_period(device: &Device, start: opcua::types::data_types::UtcTime,
    end: opcua::types::data_types::UtcTime, time_stamp: TimestampsToReturn, 
    bounds: bool, num_values_per_node: u32) -> (Option<Vec<opcua::types::data_value::DataValue>>, opcua_types::StatusCode) {
    
    let mut result: Vec<opcua::types::data_value::DataValue> = Vec::new();
    let mut status_result = opcua_types::StatusCode::BadCommunicationError;

    //println!("start {:?} end {:?}", start, end);
    //println!("epoch {:?} endtimes {:?}", opcua_types::DateTime::epoch(), opcua_types::DateTime::endtimes());
    if !bounds && num_values_per_node == 1 {
        if start == opcua_types::DateTime::epoch() && end == opcua_types::DateTime::endtimes() {
            //last point
            return request_ursv5xx_end_point(device, time_stamp);
        }
        else if start == opcua_types::DateTime::epoch() + chrono::TimeDelta::seconds(1) && end == opcua_types::DateTime::epoch() {
            //oldest point
            return request_ursv5xx_start_point(device, time_stamp);
        }
    }
    
    match get_period(start, end, bounds, num_values_per_node) {
        Some(period) => {
            for time in period {
                let answer = match request_ursv5xx_at_time(device, chrono::DateTime::<Local>::from(time.as_chrono()), time_stamp) {
                    Ok(v) => {
                        status_result = opcua_types::StatusCode::Good;
                        v
                    },
                    Err(_) => {let mut v = opcua::types::data_value::DataValue::new_at_status(0, time, 
                            opcua_types::StatusCode::BadCommunicationError);
                        v.set_timestamps(time_stamp, time, time);
                        v
                    },
                };
                result.push(answer);
                sleep(Duration::from_millis(device.history_read_await));
            };
            (Some(result), status_result)
        },
        None => (None, status_result),
    }
}

fn get_period(start: opcua::types::data_types::UtcTime, end: opcua::types::data_types::UtcTime, 
    bounds: bool, num_values_per_node: u32) -> Option<Vec<opcua::types::data_types::UtcTime>> {
    
    let mut result: Vec<opcua::types::data_types::UtcTime> = Vec::new();
    let opc_end = opcua_types::DateTime::epoch();
    
    if (((start == opc_end) && (num_values_per_node != 0)) || (start > end)) && (end != opc_end){
        let mut counter = num_values_per_node;
         let mut time = end;       
        if !bounds { time = time.sub(chrono::Duration::hours(1)); }
        while counter > 0 {
            result.push(time);
            time = time.sub(chrono::Duration::hours(1));            
            counter -= 1;
        }
    }
    else if end == opc_end && num_values_per_node != 0 {
        let mut counter = num_values_per_node;
         let mut time = start;       
        if !bounds { time = time.add(chrono::Duration::hours(1)); }
        while counter > 0 {
            result.push(time);
            time = time.add(chrono::Duration::hours(1));
            counter -= 1;
        }
    }
    else {
        let mut time = start;
        let mut stop_time = end;
        if stop_time.as_chrono() > Utc::now() {
            stop_time = opcua_types::DateTime::from(Utc::now());
        }
        if !bounds {
            time = start.add(chrono::Duration::hours(1));
        }
        while time < stop_time {
            result.push(time);
            time = time.add(chrono::Duration::hours(1));
        }
        if bounds {
            result.push(stop_time);
        }
        let sz = match usize::try_from(num_values_per_node) {
            Ok(v) => v,
            Err(_) => 0,} ;
        if num_values_per_node != 0 && result.len() >= sz{
            result.truncate(sz);
        }
    }
    if result.len() == 0 {
        return None
    }
    Some(result)
}

fn request_ursv5xx_at_time(device_lite_m: &Device, request_time: chrono::DateTime<chrono::Local>, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, io::Error> {
    let mut request: [u8; 19] = [0, 0, 0, 0, 0, 0x0D, 0, 0x41, 0, 0x00, 0, 0x01, 0x01, 0, 0, 0, 0, 0, 0];
    request[6] = device_lite_m.device_address;
    let session_id: u16 = random::<u16>();
    request[0..2].copy_from_slice(&session_id.to_be_bytes());
    request[15] = u8::from(request_time.hour().to_be_bytes()[3]);
    request[16] = u8::from(request_time.day().to_be_bytes()[3]);
    request[17] = u8::from(request_time.month().to_be_bytes()[3]);
    request[18] = u8::from((request_time.year() - 2000).to_be_bytes()[3]);

    let mut str = std::net::TcpStream::connect_timeout(&device_lite_m.device_ip_address.parse::<core::net::SocketAddr>().unwrap(), 
        Duration::from_millis(device_lite_m.connection_timeout))?;

    str.write(&request)?;
    
    read_arc_answer(session_id, device_lite_m, str, time_stamp)    
}

fn request_ursv5xx_by_index(device_lite_m: &Device, request_index: u16, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, io::Error> {
    let mut request: [u8; 15] = [0, 0, 0, 0, 0, 0x0D, 0, 0x41, 0, 0x00, 0, 0x01, 0x00, 0, 0];
    request[6] = device_lite_m.device_address;
    let session_id: u16 = random::<u16>();
    request[0..2].copy_from_slice(&session_id.to_be_bytes());
    request[12..].copy_from_slice(&request_index.to_be_bytes());

    let mut str = std::net::TcpStream::connect_timeout(&device_lite_m.device_ip_address.parse::<core::net::SocketAddr>().unwrap(), 
        Duration::from_millis(device_lite_m.connection_timeout))?;

    str.write(&request)?;

    read_arc_answer(session_id, device_lite_m, str, time_stamp) 
}

fn read_arc_answer(session_id: u16, device_lite_m: &Device, mut str: TcpStream, time_stamp: TimestampsToReturn) -> Result<data_value::DataValue, io::Error> {
    str.set_read_timeout(Some(Duration::from_millis(device_lite_m.read_timeout)))?;
 
    let mut answer: [u8; 72]= [0; 72];
    match str.read(&mut answer) {
        Ok(_) => {
            let id = u16::from_be_bytes([answer[0], answer[1]]);
            if id == session_id{
                let mut staus_code = opcua_types::StatusCode::Good;
                let time_source = u32::from_be_bytes([answer[9], answer[10], answer[11], answer[12]]);
                if time_source == 0 {
                    staus_code = opcua_types::StatusCode::BadNoData;
                }
                let t = chrono::DateTime::from_timestamp_secs(i64::from(time_source))
                    .unwrap().sub(Local::now().offset().fix());//.with_timezone(&Local);
                let mut val = data_value::DataValue::new_at_status( get_ursv5xx_volume(&answer[17..26]), 
                    opcua_types::DateTime::from(chrono::DateTime::<Utc>::from(t)), staus_code);
                val.set_timestamps(time_stamp, 
                    val.source_timestamp.unwrap(), 
                    opcua_types::DateTime::from(Utc::now()));
                Ok(val)
            }
            else {
                Err(io::Error::new(io::ErrorKind::Other, "Bad session id"))
            }
        }
        Err(e) => Err(e),
    }    
}

fn get_ursv5xx_volume(buffer: &[u8]) -> f32
{
    let positive_f = f32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
    let negative_f = f32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

    positive_f + negative_f
}