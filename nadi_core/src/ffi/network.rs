use crate::ffi::*;
use crate::network::*;
use crate::node::*;
use std::ffi::CStr;
use std::os::raw::c_uint;

#[no_mangle]
pub extern "C" fn nadi_network_nodes_count(network: *const Network) -> c_uint {
    let network = unsafe { &*network };
    network.nodes_count() as c_uint
}

#[no_mangle]
pub extern "C" fn nadi_network_node(network: *const Network, index: c_uint) -> *const NodeInner {
    let network = unsafe { &*network };
    match network.node(index as usize) {
        Some(node) => node.as_ptr() as *const NodeInner,
        None => null(),
    }
}

#[no_mangle]
pub extern "C" fn nadi_network_node_by_name(
    network: *const Network,
    name: *const c_char,
) -> *const NodeInner {
    let network = unsafe { &*network };
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    match network.node_by_name(&name) {
        Some(node) => node.as_ptr() as *const NodeInner,
        None => null(),
    }
}

#[no_mangle]
pub extern "C" fn nadi_network_output(network: *const Network) -> *const NodeInner {
    let network = unsafe { &*network };
    match network.output() {
        Some(node) => node.as_ptr() as *const NodeInner,
        None => null(),
    }
}
