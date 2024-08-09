use crate::attributes::AsValue;
use std::ffi::CString;
use std::os::raw::{c_char, c_float, c_int, c_uint};
use std::ptr::null;

mod context;
mod network;
mod node;
mod timeseries;

// The pointers returned to c will be *const, as they are not meant to
// be modified. To free them the Box::from_raw needs them to be *mut,
// so that conversion is done later.
trait AsCPtrs: AsValue {
    fn into_toml_string_ptr(self) -> *const c_char;
    fn into_string_ptr(self) -> *const c_char;
    fn into_int_ptr(self) -> *const c_int;
    fn into_float_ptr(self) -> *const c_float;
}

impl AsCPtrs for toml::Value {
    fn into_toml_string_ptr(self) -> *const c_char {
        self.into_toml_string().map(into_c_str).unwrap_or_else(null)
    }
    fn into_string_ptr(self) -> *const c_char {
        self.into_string().map(into_c_str).unwrap_or_else(null)
    }
    fn into_int_ptr(self) -> *const c_int {
        self.into_loose_int()
            .map(|i| into_c_ptr(i as c_int))
            .unwrap_or_else(null)
    }
    fn into_float_ptr(self) -> *const c_float {
        self.into_loose_float()
            .map(|f| into_c_ptr(f as c_float))
            .unwrap_or_else(null)
    }
}

impl AsCPtrs for Option<toml::Value> {
    fn into_toml_string_ptr(self) -> *const c_char {
        self.map(AsCPtrs::into_toml_string_ptr).unwrap_or_else(null)
    }
    fn into_string_ptr(self) -> *const c_char {
        self.map(AsCPtrs::into_string_ptr).unwrap_or_else(null)
    }
    fn into_int_ptr(self) -> *const c_int {
        self.map(AsCPtrs::into_int_ptr).unwrap_or_else(null)
    }
    fn into_float_ptr(self) -> *const c_float {
        self.map(AsCPtrs::into_float_ptr).unwrap_or_else(null)
    }
}

fn into_c_ptr<T>(inp: T) -> *const T {
    Box::into_raw(Box::new(inp))
}

fn into_c_str(s: String) -> *const c_char {
    CString::new(s).unwrap().into_raw()
}

/// Get the nadi_core version, should be the same version as the nadi
/// system for the plugin to work.
#[no_mangle]
pub extern "C" fn nadi_core_version() -> *const c_char {
    CString::new(crate::CORE_VERSION).unwrap().into_raw()
}

/// Get the rustc version, it should also be the same version as the
/// one nadi system was built on as rust doesn't have stable abi.
#[no_mangle]
pub extern "C" fn nadi_rustc_version() -> *const c_char {
    CString::new(crate::RUSTC_VERSION).unwrap().into_raw()
}

/// Free the pointer obtained from `nadi_node_attr_to_float`
#[no_mangle]
pub extern "C" fn nadi_free_float(ptr: *const c_float) {
    if ptr.is_null() {
        eprintln!("Trying to free Null pointer");
        return;
    }
    unsafe {
        _ = Box::from_raw(ptr as *mut c_float);
    }
}

/// Free the pointer obtained from `nadi_node_attr_to_int`
#[no_mangle]
pub extern "C" fn nadi_free_int(ptr: *const c_int) {
    if ptr.is_null() {
        eprintln!("Trying to free Null pointer");
        return;
    }
    unsafe {
        _ = Box::from_raw(ptr as *mut c_int);
    }
}

/// Call this to free the string value obtained from
/// `nadi_node_attr_to_string`
#[no_mangle]
pub extern "C" fn nadi_free_string(ptr: *const c_char) {
    if ptr.is_null() {
        eprintln!("Trying to free Null pointer");
        return;
    }
    unsafe {
        _ = CString::from_raw(ptr as *mut c_char);
    }
}

/// Call this to free the float value obtained from
/// `nadi_node_timeseries_float_values`
///
/// # Safety
///
/// Only call this function to free the float timeseries received from
/// `nadi_node_timeseries_float_values`.
#[no_mangle]
pub unsafe extern "C" fn nadi_free_float_timeseries(
    ptr: *mut c_float,
    length: c_uint,
    capacity: c_uint,
) {
    if ptr.is_null() {
        eprintln!("Trying to free Null pointer");
        return;
    }
    unsafe {
        _ = Vec::from_raw_parts(ptr, length as usize, capacity as usize);
    }
}

/// Call this to free the int value obtained from
/// `nadi_node_timeseries_int_values`
///
/// # Safety
///
/// Only call this function to free the int timeseries received from
/// `nadi_node_timeseries_int_values`.
#[no_mangle]
pub unsafe extern "C" fn nadi_free_int_timeseries(
    ptr: *mut c_int,
    length: c_uint,
    capacity: c_uint,
) {
    if ptr.is_null() {
        eprintln!("Trying to free Null pointer");
        return;
    }
    unsafe {
        _ = Vec::from_raw_parts(ptr, length as usize, capacity as usize);
    }
}
