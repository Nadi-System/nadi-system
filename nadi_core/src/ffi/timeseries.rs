use anyhow::Context;

use crate::ffi::*;
use crate::node::NodeInner;
use crate::timeseries::{LoadedTimeSeries, TimeSeries, TimeSeriesData};
use std::ffi::CStr;
use std::mem;
use std::os::raw::c_uint;
use std::ptr::null_mut;

/// get the timeseries length, 0 if no such timeseries
#[no_mangle]
pub extern "C" fn nadi_node_timeseries_length(node: *mut NodeInner, name: *const c_char) -> c_uint {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    match node.timeseries(&name) {
        Ok(ts) => ts.length() as c_uint,
        Err(e) => {
            eprintln!("{:?}", e);
            0
        }
    }
}

/// get the timeseries start as string, null if no such timeseries
#[no_mangle]
pub extern "C" fn nadi_node_timeseries_start(
    node: *mut NodeInner,
    name: *const c_char,
) -> *const c_char {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    match node.timeseries(&name) {
        Ok(ts) => into_c_str(ts.start().to_string()),
        Err(e) => {
            eprintln!("{:?}", e);
            null()
        }
    }
}

/// get the timeseries timestep in seconds, null if no such timeseries
#[no_mangle]
pub extern "C" fn nadi_node_timeseries_timestep(
    node: *mut NodeInner,
    name: *const c_char,
) -> c_uint {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    match node.timeseries(&name) {
        Ok(ts) => ts.timestep().as_secs() as c_uint,
        Err(e) => {
            eprintln!("{:?}", e);
            0
        }
    }
}

/// check if the timeseries is present in a node
#[no_mangle]
pub extern "C" fn nadi_node_timeseries_exists(node: *mut NodeInner, name: *const c_char) -> bool {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    node.has_timeseries(&name)
}

// /// get the string timeseries from the node, null if no such timeseries
// ///
// /// The value needs to be freed using `nadi_free_string`, do not call
// /// `free()` from c.
// #[no_mangle]
// pub extern "C" fn nadi_node_timeseries_string_values(
//     node: *mut NodeInner,
//     name: *const c_char,
// ) -> *const *const c_char {
//     let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
//     let node = unsafe { &mut *node };
//     match node.timeseries(&name).ok().and_then(|ts| ts.values_str()) {
//         Some(ts) => {
//             let cstrs: Vec<*const c_char> =
//                 ts.iter().map(|s| s.to_string()).map(into_c_str).collect();
//             Box::into_raw(cstrs.into_boxed_slice()) as *const *const c_char
//         }
//         None => null(),
//     }
// }

/// get the float timeseries from the node, null if no such timeseries
///
/// The value needs to be freed using `nadi_free_float`, do not call
/// `free()` from c.
#[no_mangle]
pub extern "C" fn nadi_node_list_timeseries(node: *mut NodeInner) {
    let node = unsafe { &mut *node };
    node.list_timeseries();
}
/// get the float timeseries from the node, null if no such timeseries
///
/// The value needs to be freed using `nadi_free_float`, do not call
/// `free()` from c.
#[no_mangle]
pub extern "C" fn nadi_node_timeseries_float_values(
    node: *mut NodeInner,
    name: *const c_char,
    length: *mut c_uint,
    capacity: *mut c_uint,
) -> *mut c_float {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    let length = unsafe { &mut *length };
    let capacity = unsafe { &mut *capacity };
    match node
        .timeseries(&name)
        .and_then(|ts| ts.values_float().context("Not Float Values"))
    {
        Ok(ts) => {
            let mut vals: Vec<c_float> = ts.iter().map(|&f| *f as c_float).collect();
            *length = vals.len() as c_uint;
            *capacity = vals.capacity() as c_uint;
            let p = vals.as_mut_ptr();
            mem::forget(vals);
            p
        }
        Err(e) => {
            eprintln!("{:?}", e);
            null_mut()
        }
    }
}

/// get the int timeseries from the node, null if no such timeseries
///
/// The value needs to be freed using `nadi_free_int`, do not call
/// `free()` from c.
#[no_mangle]
pub extern "C" fn nadi_node_timeseries_int_values(
    node: *mut NodeInner,
    name: *const c_char,
    length: *mut c_uint,
    capacity: *mut c_uint,
) -> *mut c_int {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    let length = unsafe { &mut *length };
    let capacity = unsafe { &mut *capacity };
    match node
        .timeseries(&name)
        .and_then(|ts| ts.values_int().context("Not Int values"))
    {
        Ok(ts) => {
            let mut vals: Vec<c_int> = ts.0.iter().map(|&f| *f as c_int).collect();
            *length = vals.len() as c_uint;
            *capacity = vals.capacity() as c_uint;
            let p = vals.as_mut_ptr();
            mem::forget(vals);
            p
        }
        Err(e) => {
            eprintln!("{:?}", e);
            null_mut()
        }
    }
}

/// Call this to return the free the float value obtained from
/// `nadi_node_timeseries_float_values` and free it
#[no_mangle]
pub extern "C" fn nadi_return_float_timeseries(
    node: *mut NodeInner,
    name: *const c_char,
    ptr: *mut c_float,
    length: c_uint,
    capacity: c_uint,
) {
    if ptr.is_null() {
        eprintln!("Trying to return Null pointer");
        return;
    }
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    let values = unsafe { Vec::from_raw_parts(ptr, length as usize, capacity as usize) };
    let ts = LoadedTimeSeries::like(
        node.timeseries(&name)
            .expect("Return should be called for existing timeseries"),
        TimeSeriesData::floats(values.into_iter().map(|f| f as f64).collect()),
    );
    node.set_timeseries(&name, ts);
}

/// Call this to free the int value obtained from
/// `nadi_node_timeseries_int_values`
#[no_mangle]
pub extern "C" fn nadi_return_int_timeseries(
    node: *mut NodeInner,
    name: *const c_char,
    ptr: *mut c_int,
    length: c_uint,
    capacity: c_uint,
) {
    if ptr.is_null() {
        eprintln!("Trying to return Null pointer");
        return;
    }
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    let values = unsafe { Vec::from_raw_parts(ptr, length as usize, capacity as usize) };
    let values: Vec<i64> = values.into_iter().map(|f| f as i64).collect();
    let mask: Vec<bool> = values.iter().map(|_| true).collect();
    let ts = LoadedTimeSeries::like(
        node.timeseries(&name)
            .expect("Return should be called for existing timeseries"),
        TimeSeriesData::ints(values, mask),
    );
    node.set_timeseries(&name, ts);
}
