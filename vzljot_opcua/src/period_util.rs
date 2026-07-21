// OPCUA vzljot for Rust
// SPDX-License-Identifier: MPL-2.0
// Copyright (C) 2026

//!OPC UA server for different flowmeters designed by developer "vzljot"

use std::ops::{Add, Sub};
use chrono::{prelude::*};

pub(crate) fn get_period(start: opcua::types::data_types::UtcTime, end: opcua::types::data_types::UtcTime, 
    bounds: bool, num_values_per_node: u32) -> (Option<Vec<opcua::types::data_types::UtcTime>>, Option<opcua::types::data_types::UtcTime>) {
    
    let mut result: Vec<opcua::types::data_types::UtcTime> = Vec::new();
    let mut continuation: Option<opcua::types::data_types::UtcTime>= None;
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
            if bounds {
                continuation = Some(result[sz])
            }
            else {
                continuation = Some(result[sz -1])
            }
            result.truncate(sz);
        }
    }
    if result.len() == 0 {
        return (None, None)
    }
    (Some(result), continuation)
}
