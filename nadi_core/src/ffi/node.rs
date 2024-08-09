use crate::ffi::*;
use crate::node::*;
use serde::Deserialize;
use std::ffi::CStr;
use std::os::raw::c_uint;
use string_template_plus::Template;

/// Simple print of the node for debugging purposes
#[no_mangle]
pub extern "C" fn nadi_node_print(node: *const NodeInner) {
    let node = unsafe { &*node };
    node.print();
}

/// Name of the node
#[no_mangle]
pub extern "C" fn nadi_node_name(node: *const NodeInner) -> *const c_char {
    let node = unsafe { &*node };
    into_c_str(node.name().to_string())
}

/// Index of the node
#[no_mangle]
pub extern "C" fn nadi_node_index(node: *const NodeInner) -> c_uint {
    let node = unsafe { &*node };
    node.index() as c_uint
}

/// Level of the node
#[no_mangle]
pub extern "C" fn nadi_node_level(node: *const NodeInner) -> c_uint {
    let node = unsafe { &*node };
    node.level() as c_uint
}
/// Order of the node
#[no_mangle]
pub extern "C" fn nadi_node_order(node: *const NodeInner) -> c_uint {
    let node = unsafe { &*node };
    node.order() as c_uint
}

/// Render the given string template for current node
#[no_mangle]
pub extern "C" fn nadi_node_render(
    node: *const NodeInner,
    template: *const c_char,
) -> *const c_char {
    let node = unsafe { &*node };
    let template = unsafe { CStr::from_ptr(template).to_string_lossy() };
    match Template::parse_template(&template).and_then(|templ| node.render(&templ)) {
        Ok(text) => into_c_str(text),
        Err(e) => {
            eprintln!("{:?}", e);
            null()
        }
    }
}

/// Print the attributes of the node in toml format
#[no_mangle]
pub extern "C" fn nadi_node_print_attrs(node: *const NodeInner) {
    let node = unsafe { &*node };
    node.print_attrs();
}

/// number of input nodes in a node
#[no_mangle]
pub extern "C" fn nadi_node_inputs_count(node: *const NodeInner) -> c_uint {
    let node = unsafe { &*node };
    node.inputs().count() as c_uint
}

/// get the input node at `index`
#[no_mangle]
pub extern "C" fn nadi_node_input(node: *const NodeInner, index: c_uint) -> *const NodeInner {
    let node = unsafe { &*node };
    match node.input(index as usize) {
        Some(node) => node.as_ptr() as *const NodeInner,
        None => null(),
    }
}

/// get the output node, null pointer if there is no output node
#[no_mangle]
pub extern "C" fn nadi_node_output(node: *const NodeInner) -> *const NodeInner {
    let node = unsafe { &*node };
    match node.output() {
        Some(node) => node.as_ptr() as *const NodeInner,
        None => null(),
    }
}

/// check if attribute exists in the node
#[no_mangle]
pub extern "C" fn nadi_node_attr_exists(node: *const NodeInner, name: *const c_char) -> bool {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &*node };
    node.attr(&name).is_some()
}

/// get the node attribute as toml compatible string, null if no attibute
///
///
/// for e.g. string values will be "quoted". The value needs to be
/// freed using `nadi_free_string`, do not call `free()` from c.
#[no_mangle]
pub extern "C" fn nadi_node_attr_as_toml_string(
    node: *const NodeInner,
    name: *const c_char,
) -> *const c_char {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &*node };
    node.attr(&name).into_string_ptr()
}

/// get the node attribute as string; returns null if attribute is not
/// string type.
///
/// The value needs to be freed using `nadi_free_string`, do not
/// call `free()` from c.
#[no_mangle]
pub extern "C" fn nadi_node_attr_as_string(
    node: *const NodeInner,
    name: *const c_char,
) -> *const c_char {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &*node };
    node.attr(&name).into_string_ptr()
}

/// get the node attribute as int for int and bool value, else returns null. The value needs to be freed
/// using `nadi_free_int`, do not call `free()` from c.
#[no_mangle]
pub extern "C" fn nadi_node_attr_as_int(
    node: *const NodeInner,
    name: *const c_char,
) -> *const c_int {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &*node };
    node.attr(&name).into_int_ptr()
}

/// get the node attribute as float for float, int and bool, else
/// returns null. The value needs to be freed using
/// `nadi_free_float`, do not call `free()` from c.
#[no_mangle]
pub extern "C" fn nadi_node_attr_as_float(
    node: *const NodeInner,
    name: *const c_char,
) -> *const c_float {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &*node };
    node.attr(&name).into_float_ptr()
}

/// Get the node attribute as boolean, for boolean, it retains the
/// value, for integer and float, non-zero are returned as true, and
/// for other types of values any value is true. returns false if the
/// attribute is not found.
#[no_mangle]
pub extern "C" fn nadi_node_attr_as_bool(node: *const NodeInner, name: *const c_char) -> bool {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &*node };
    node.attr(&name).into_loose_bool()
}

/// Set the given as string as a string node attribute
#[no_mangle]
pub extern "C" fn nadi_set_node_attr_as_string(
    // so far there is no way to get *mut NodeInner from C api without
    // making a new node.
    node: *mut NodeInner,
    name: *const c_char,
    value: *const c_char,
) {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let value = unsafe { CStr::from_ptr(value).to_string_lossy() };
    let node = unsafe { &mut *node };
    node.set_attr(name, toml::Value::String(value.to_string()));
}

/// Parse the provided string and set that as the node attribute.
///
/// The provided string should be in toml values format, e.g. strings
/// should be quoted.
#[no_mangle]
pub extern "C" fn nadi_set_node_attr_from_string(
    // so far there is no way to get *mut NodeInner from C api except through the plugin
    node: *mut NodeInner,
    name: *const c_char,
    value: *const c_char,
) -> bool {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let value = unsafe { CStr::from_ptr(value).to_string_lossy() };
    let node = unsafe { &mut *node };
    match toml::Value::deserialize(toml::de::ValueDeserializer::new(value.as_ref())) {
        Ok(v) => {
            node.set_attr(name, v);
            true
        }
        Err(e) => {
            eprintln!("{:?}", e);
            false
        }
    }
}

/// Set the node attribute as given float value
#[no_mangle]
pub extern "C" fn nadi_set_node_attr_as_float(
    // so far there is no way to get *mut NodeInner from C api without
    // making a new node.
    node: *mut NodeInner,
    name: *const c_char,
    value: c_float,
) {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    node.set_attr(name, toml::Value::Float(value as f64));
}

/// Set the node attribute as given bool value
#[no_mangle]
pub extern "C" fn nadi_set_node_attr_as_bool(
    // so far there is no way to get *mut NodeInner from C api without
    // making a new node.
    node: *mut NodeInner,
    name: *const c_char,
    value: bool,
) {
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    let node = unsafe { &mut *node };
    node.set_attr(name, toml::Value::Boolean(value));
}
