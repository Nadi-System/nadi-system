use crate::ffi::*;
use crate::plugins::FunctionCtx;
use std::ffi::CStr;

/// Set error string in the `FunctionCtx`, will be read to determine
/// if function call was successful or not
#[no_mangle]
pub extern "C" fn nadi_ctx_set_error(ctx: *mut FunctionCtx, error: *const c_char) {
    let ctx = unsafe { &mut *ctx };
    let error = unsafe { CStr::from_ptr(error).to_string_lossy() };
    ctx.set_error(anyhow::Error::msg(error.to_string()))
}

/// Get the error string in the `FunctionCtx` if present, null otherwise
#[no_mangle]
pub extern "C" fn nadi_ctx_error(ctx: *const FunctionCtx) -> *const c_char {
    let ctx = unsafe { &*ctx };
    ctx.error().map(toml::Value::String).into_string_ptr()
}

/// Get the Number of arguments in the `FunctionCtx`
#[no_mangle]
pub extern "C" fn nadi_ctx_args_count(ctx: *const FunctionCtx) -> c_uint {
    let ctx = unsafe { &*ctx };
    ctx.args_count() as c_uint
}

/// Get the argument in the `FunctionCtx` as toml string
#[no_mangle]
pub extern "C" fn nadi_ctx_arg_to_toml_string(
    ctx: *const FunctionCtx,
    ind: c_uint,
) -> *const c_char {
    let ctx = unsafe { &*ctx };
    ctx.arg(ind as usize).cloned().into_toml_string_ptr()
}

/// Get the argument in the `FunctionCtx` if string
#[no_mangle]
pub extern "C" fn nadi_ctx_arg_to_string(ctx: *const FunctionCtx, ind: c_uint) -> *const c_char {
    let ctx = unsafe { &*ctx };
    ctx.arg(ind as usize).cloned().into_string_ptr()
}

/// Get the argument in the `FunctionCtx` as int if it's int or bool
#[no_mangle]
pub extern "C" fn nadi_ctx_arg_to_int(ctx: *const FunctionCtx, ind: c_uint) -> *const c_int {
    let ctx = unsafe { &*ctx };
    ctx.arg(ind as usize).cloned().into_int_ptr()
}

/// Get the argument in the `FunctionCtx` as float if it's float,
/// int, or bool
#[no_mangle]
pub extern "C" fn nadi_ctx_arg_to_float(ctx: *const FunctionCtx, ind: c_uint) -> *const c_float {
    let ctx = unsafe { &*ctx };
    ctx.arg(ind as usize).cloned().into_float_ptr()
}

/// Get the argument in the `FunctionCtx` as bool
#[no_mangle]
pub extern "C" fn nadi_ctx_arg_to_bool(ctx: *const FunctionCtx, ind: c_uint) -> bool {
    let ctx = unsafe { &*ctx };
    ctx.arg(ind as usize).cloned().into_loose_bool()
}

/// Get whether or not keyword argument is in the `FunctionCtx`
#[no_mangle]
pub extern "C" fn nadi_ctx_kwarg_exists(ctx: *const FunctionCtx, name: *const c_char) -> bool {
    let ctx = unsafe { &*ctx };
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    ctx.kwarg(&name).is_some()
}

/// Get the keyword argument in the `FunctionCtx` as toml string
#[no_mangle]
pub extern "C" fn nadi_ctx_kwarg_to_toml_string(
    ctx: *const FunctionCtx,
    name: *const c_char,
) -> *const c_char {
    let ctx = unsafe { &*ctx };
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    ctx.kwarg(&name).cloned().into_toml_string_ptr()
}

/// Get the keyword argument in the `FunctionCtx` as string
#[no_mangle]
pub extern "C" fn nadi_ctx_kwarg_to_string(
    ctx: *const FunctionCtx,
    name: *const c_char,
) -> *const c_char {
    let ctx = unsafe { &*ctx };
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    ctx.kwarg(&name).cloned().into_string_ptr()
}

/// Get the keyword argument in the `FunctionCtx` as int if int or bool
#[no_mangle]
pub extern "C" fn nadi_ctx_kwarg_to_int(
    ctx: *const FunctionCtx,
    name: *const c_char,
) -> *const c_int {
    let ctx = unsafe { &*ctx };
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    ctx.kwarg(&name).cloned().into_int_ptr()
}

/// Get the keyword argument in the `FunctionCtx` as float if
/// float, int or bool
#[no_mangle]
pub extern "C" fn nadi_ctx_kwarg_to_float(
    ctx: *const FunctionCtx,
    name: *const c_char,
) -> *const c_float {
    let ctx = unsafe { &*ctx };
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    ctx.kwarg(&name).cloned().into_float_ptr()
}

/// Get the keyword argument in the `FunctionCtx` as bool
#[no_mangle]
pub extern "C" fn nadi_ctx_kwarg_to_bool(ctx: *const FunctionCtx, name: *const c_char) -> bool {
    let ctx = unsafe { &*ctx };
    let name = unsafe { CStr::from_ptr(name).to_string_lossy() };
    ctx.kwarg(&name).cloned().into_loose_bool()
}
