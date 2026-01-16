/* src/runtime.rs */

use crate::environment::GateDefinition;
use crate::environment::{Environment, RuntimeValue};
use crate::evaluator::Evaluator;
use std::alloc::{alloc, Layout};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::io::Write;
use std::os::raw::c_int;
use std::rc::Rc;
use std::slice;

type StatePtr = *mut c_void;

#[no_mangle]
pub extern "C" fn quantica_rt_new_state(num_qubits: c_int) -> StatePtr {
    let mut state_map: HashMap<usize, (f64, f64)> = HashMap::new();
    state_map.insert(0, (1.0, 0.0));

    let register = RuntimeValue::QuantumRegister {
        state: Rc::new(RefCell::new(state_map)),
        size: num_qubits as usize,
        register_name: "runtime_reg".to_string(),
        global_start_index: None,
    };

    let boxed_register = Box::new(register);
    Box::into_raw(boxed_register) as StatePtr
}

#[no_mangle]
pub extern "C" fn quantica_rt_measure(state_ptr: StatePtr, qubit_index: c_int) -> c_int {
    if state_ptr.is_null() {
        eprintln!("(Runtime Error) quantica_rt_measure called with null pointer.");
        return -1;
    }

    let register = unsafe { &*(state_ptr as *mut RuntimeValue) };

    match register {
        RuntimeValue::QuantumRegister { state, size, .. } => {
            let idx = qubit_index as usize;
            if idx >= *size {
                eprintln!("(Runtime Error) Qubit index {} out of bounds (size {}).", idx, size);
                return -1;
            }

            let qubit_handle = RuntimeValue::Qubit {
                state: state.clone(),
                index: idx,
                size: *size,
                register_name: "runtime_reg".to_string(),
                global_index: None,
            };

            let args = vec![qubit_handle];

            match Evaluator::builtin_measure(args) {
                Ok(RuntimeValue::Int(val)) => val as c_int,
                Ok(_) => -1,
                Err(e) => {
                    eprintln!("(Runtime Error) Measurement failed: {}", e);
                    -1
                }
            }
        }
        _ => {
            eprintln!("(Runtime Error) Invalid state pointer passed to measure.");
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn quantica_rt_device_alloc(size_bytes: usize) -> *mut c_void {
    if size_bytes == 0 {
        return std::ptr::null_mut();
    }

    let layout = Layout::from_size_align(size_bytes, 8).unwrap();
    let ptr = unsafe { alloc(layout) as *mut c_void };

    if ptr.is_null() {
        eprintln!("(Runtime Error) Device memory allocation failed for {} bytes.", size_bytes);
    }

    ptr
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_device_free(device_ptr: *mut c_void) {
    if device_ptr.is_null() { return; }



}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_htod_transfer(host_ptr: *const c_void, device_ptr: *mut c_void, size_bytes: usize) -> c_int {
    unsafe { std::ptr::copy_nonoverlapping(host_ptr, device_ptr, size_bytes); }
    0
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_dtoh_transfer(host_ptr: *mut c_void, device_ptr: *const c_void, size_bytes: usize) -> c_int {
    unsafe { std::ptr::copy_nonoverlapping(device_ptr, host_ptr, size_bytes); }
    0
}

#[no_mangle]
pub extern "C" fn quantica_rt_debug_state(state_ptr: StatePtr) {
    if state_ptr.is_null() {
        println!("(Runtime Error) quantica_rt_debug_state called with null pointer.");
        return;
    }
    let register = unsafe { &*(state_ptr as *mut RuntimeValue) };
    if let RuntimeValue::QuantumRegister { state, size, .. } = register {
        println!("(Quantum Runtime) Debugging state ({} qubits):", size);
        Evaluator::print_quantum_state(state, *size, 10);
    } else {
        println!("(Runtime Error) Invalid state pointer passed to debug_state.");
    }
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_apply_gate(
    state_ptr: StatePtr,
    gate_name_ptr: *const c_char,
    is_dagger_int: c_int,
    params_ptr: *const f64,
    num_params: c_int,
    qubit_indices_ptr: *const c_int,
    num_qubits: c_int,
    num_controls: c_int,
) -> c_int {
    match unsafe {
        apply_gate_unsafe(state_ptr, gate_name_ptr, is_dagger_int, params_ptr, num_params, qubit_indices_ptr, num_qubits, num_controls)
    } {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("(Quantum Runtime Error) {}", e);
            1
        }
    }
}

unsafe fn apply_gate_unsafe(
    state_ptr: StatePtr,
    gate_name_ptr: *const c_char,
    is_dagger_int: c_int,
    params_ptr: *const f64,
    num_params: c_int,
    qubit_indices_ptr: *const c_int,
    num_qubits: c_int,
    num_controls: c_int,
) -> Result<RuntimeValue, String> {
    if state_ptr.is_null() { return Err("State pointer is null.".to_string()); }
    let register = &*(state_ptr as *mut RuntimeValue);
    let (state_rc, reg_size) = match register {
        RuntimeValue::QuantumRegister { state, size, .. } => (state.clone(), *size),
        _ => return Err("Invalid state pointer passed to apply_gate.".to_string()),
    };

    let gate_name_cstr = CStr::from_ptr(gate_name_ptr);
    let gate_name = gate_name_cstr.to_str().unwrap_or("").to_string();
    let is_dagger = is_dagger_int != 0;

    let params: Vec<f64> = if params_ptr.is_null() { Vec::new() } else { slice::from_raw_parts(params_ptr, num_params as usize).to_vec() };
    let qubit_indices: Vec<usize> = if qubit_indices_ptr.is_null() { Vec::new() } else { slice::from_raw_parts(qubit_indices_ptr, num_qubits as usize).iter().map(|&x| x as usize).collect() };

    if (num_controls as usize) > qubit_indices.len() { return Err("More controls specified than total qubits.".to_string()); }
    let (controls, targets) = qubit_indices.split_at(num_controls as usize);

    let gate_def = GateDefinition {
        name: gate_name,
        params: params,
        controls: controls.to_vec(),
        targets: targets.to_vec(),
        register_size: reg_size,
        state_rc: state_rc,
    };

    Evaluator::apply_multi_controlled_gate(gate_def, is_dagger)
}

#[no_mangle]
pub extern "C" fn quantica_rt_print_int(n: i64) {
    println!("{}", n);
    let _ = std::io::stdout().flush();
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_print_string(s: *const c_char) {
    if s.is_null() {
        println!("(null)");
    } else {
        let c_str = unsafe { CStr::from_ptr(s) };
        if let Ok(str_slice) = c_str.to_str() {
            println!("{}", str_slice);
        } else {
            println!("(invalid utf8)");
        }
    }
    let _ = std::io::stdout().flush();
}

#[no_mangle]
pub extern "C" fn quantica_rt_print_float(val: f64) {
    println!("{:.6}", val);
    let _ = std::io::stdout().flush();
}

#[no_mangle]
pub extern "C" fn quantica_rt_int_to_string(val: i64) -> *mut c_char {
    let s = format!("{}", val);
    CString::new(s).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn quantica_rt_float_to_string(val: f64) -> *mut c_char {
    let s = format!("{:.6}", val);
    CString::new(s).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_string_concat(s1: *const c_char, s2: *const c_char) -> *mut c_char {
    let str1 = if s1.is_null() { "" } else { CStr::from_ptr(s1).to_str().unwrap_or("") };
    let str2 = if s2.is_null() { "" } else { CStr::from_ptr(s2).to_str().unwrap_or("") };

    let result = format!("{}{}", str1, str2);
    CString::new(result).unwrap().into_raw()
}


#[no_mangle]
pub unsafe extern "C" fn quantica_rt_string_cmp(s1: *const c_char, s2: *const c_char) -> i32 {
    if s1 == s2 { return 0; }
    if s1.is_null() { return -1; }
    if s2.is_null() { return 1; }

    let str1 = CStr::from_ptr(s1);
    let str2 = CStr::from_ptr(s2);

    if str1 == str2 { 0 } else { 1 }
}

#[no_mangle]
pub extern "C" fn quantica_rt_time() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(n) => n.as_secs_f64(),
        Err(_) => 0.0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_file_read(fname_ptr: *const c_char) -> *mut c_char {
    if fname_ptr.is_null() { return std::ptr::null_mut(); }

    let c_str = CStr::from_ptr(fname_ptr);
    let fname = c_str.to_string_lossy().into_owned();


    let content = match std::fs::read_to_string(&fname) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("(Runtime Error) Failed to read file '{}': {}", fname, e);
            String::new()
        }
    };

    let c_content = CString::new(content).unwrap();
    c_content.into_raw()
}
#[no_mangle]
pub unsafe extern "C" fn quantica_rt_file_write(fname_ptr: *const c_char, content_ptr: *const c_char) {
    if fname_ptr.is_null() || content_ptr.is_null() { return; }
    let fname = CStr::from_ptr(fname_ptr).to_string_lossy();
    let content = CStr::from_ptr(content_ptr).to_string_lossy();
    let _ = std::fs::write(fname.as_ref(), content.as_ref());
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_matrix_update(
    w_ptr: *mut *mut f64,
    d_ptr: *const f64,
    i_ptr: *const f64,
    lr: f64,
    rows: i32,
    cols: i32
) {



    if w_ptr.is_null() || d_ptr.is_null() || i_ptr.is_null() {
        return;
    }

    let rows = rows as usize;
    let cols = cols as usize;

    for r in 0..rows {


        let row_data_ptr = *w_ptr.add(r);

        if row_data_ptr.is_null() { continue; }

        let d_val = *d_ptr.add(r);

        for c in 0..cols {

            let weight_ptr = row_data_ptr.add(c);

            let input_val = *i_ptr.add(c);
            let old_w = *weight_ptr;
            let delta = lr * d_val * input_val;

            *weight_ptr = old_w - delta;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_compute_input_gradient(
    w_ptr: *const *const f64,
    d_ptr: *const f64,
    rows: i32,
    cols: i32
) -> *mut f64 {


    if w_ptr.is_null() || d_ptr.is_null() {
        return std::ptr::null_mut();
    }

    let rows = rows as usize;
    let cols = cols as usize;


    let mut input_grad = vec![0.0; cols];

    for r in 0..rows {

        let row_data_ptr = *w_ptr.add(r);
        if row_data_ptr.is_null() { continue; }

        let d_val = *d_ptr.add(r);

        for c in 0..cols {

            let w_val = *row_data_ptr.add(c);
            input_grad[c] += w_val * d_val;
        }
    }


    let mut boxed = input_grad.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_string_len(s: *const c_char) -> i64 {
    if s.is_null() { return 0; }
    CStr::from_ptr(s).to_bytes().len() as i64
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_array_len(arr_ptr: *mut u8) -> i64 {
    if arr_ptr.is_null() {
        return 0;
    }

    let size_ptr = (arr_ptr as *mut i64).offset(-1);
    *size_ptr
}

unsafe fn alloc_array_with_header(element_count: usize, element_size: usize) -> *mut c_void {
    let payload_size = element_count * element_size;
    let total_size = 8 + payload_size;

    let layout = Layout::from_size_align(total_size, 8).unwrap();
    let raw_ptr = alloc(layout) as *mut u8;


    *(raw_ptr as *mut i64) = element_count as i64;


    raw_ptr.add(8) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_string_split(s_ptr: *const c_char, delim_ptr: *const c_char) -> *mut c_void {
    let s_cstr = CStr::from_ptr(s_ptr);
    let s_str = s_cstr.to_str().unwrap_or("");

    let delim_cstr = CStr::from_ptr(delim_ptr);
    let delim_str = delim_cstr.to_str().unwrap_or("");

    let parts: Vec<&str> = s_str.split(delim_str).collect();
    let count = parts.len();


    let array_ptr = alloc_array_with_header(count, 8) as *mut *mut c_char;

    for (i, part) in parts.iter().enumerate() {
        let c_part = CString::new(*part).unwrap();
        *array_ptr.add(i) = c_part.into_raw();
    }

    array_ptr as *mut c_void
}

#[no_mangle]
pub extern "C" fn quantica_rt_math_exp(val: f64) -> f64 { val.exp() }

#[no_mangle]
pub extern "C" fn quantica_rt_math_log(val: f64) -> f64 { val.ln() }

#[no_mangle]
pub extern "C" fn quantica_rt_math_sqrt(val: f64) -> f64 { val.sqrt() }

#[no_mangle]
pub extern "C" fn quantica_rt_math_pow(base: f64, exp: f64) -> f64 { base.powf(exp) }

#[no_mangle]
pub extern "C" fn quantica_rt_math_random() -> f64 {

    static mut SEED: u64 = 123456789;
    unsafe {
        SEED = (SEED.wrapping_mul(6364136223846793005)).wrapping_add(1);
        (SEED as f64) / (u64::MAX as f64)
    }
}
#[no_mangle] pub extern "C" fn quantica_rt_math_sin(x: f64) -> f64 { x.sin() }
#[no_mangle] pub extern "C" fn quantica_rt_math_cos(x: f64) -> f64 { x.cos() }
#[no_mangle] pub extern "C" fn quantica_rt_math_tan(x: f64) -> f64 { x.tan() }
#[no_mangle] pub extern "C" fn quantica_rt_math_asin(x: f64) -> f64 { x.asin() }
#[no_mangle] pub extern "C" fn quantica_rt_math_acos(x: f64) -> f64 { x.acos() }
#[no_mangle] pub extern "C" fn quantica_rt_math_atan(x: f64) -> f64 { x.atan() }
#[no_mangle] pub extern "C" fn quantica_rt_math_atan2(y: f64, x: f64) -> f64 { y.atan2(x) }


#[no_mangle] pub extern "C" fn quantica_rt_math_sinh(x: f64) -> f64 { x.sinh() }
#[no_mangle] pub extern "C" fn quantica_rt_math_cosh(x: f64) -> f64 { x.cosh() }
#[no_mangle] pub extern "C" fn quantica_rt_math_tanh(x: f64) -> f64 { x.tanh() }


#[no_mangle] pub extern "C" fn quantica_rt_math_abs(x: f64) -> f64 { x.abs() }
#[no_mangle] pub extern "C" fn quantica_rt_math_floor(x: f64) -> f64 { x.floor() }
#[no_mangle] pub extern "C" fn quantica_rt_math_ceil(x: f64) -> f64 { x.ceil() }
#[no_mangle] pub extern "C" fn quantica_rt_math_round(x: f64) -> f64 { x.round() }
#[no_mangle] pub extern "C" fn quantica_rt_math_trunc(x: f64) -> f64 { x.trunc() }


#[no_mangle] pub extern "C" fn quantica_rt_math_log10(x: f64) -> f64 { x.log10() }
#[no_mangle] pub extern "C" fn quantica_rt_math_log2(x: f64) -> f64 { x.log2() }
#[no_mangle]
pub extern "C" fn quantica_rt_math_relu(x: f64) -> f64 {
    if x > 0.0 { x } else { 0.0 }
}

#[no_mangle]
pub extern "C" fn quantica_rt_math_sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_atoi(s: *const c_char) -> i32 {
    if s.is_null() { return 0; }
    let c_str = CStr::from_ptr(s);
    let str_slice = c_str.to_str().unwrap_or("0");
    str_slice.trim().parse::<i32>().unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_atof(s: *const c_char) -> f64 {
    if s.is_null() { return 0.0; }
    let c_str = CStr::from_ptr(s);
    let str_slice = c_str.to_str().unwrap_or("0.0");
    str_slice.trim().parse::<f64>().unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn quantica_rt_array_concat(a_ptr: *mut u8, b_ptr: *mut u8) -> *mut c_void {
    let elem_size = 8;



    let a_len = if a_ptr.is_null() { 0 } else { *(a_ptr as *mut i64).offset(-1) };
    let b_len = if b_ptr.is_null() { 0 } else { *(b_ptr as *mut i64).offset(-1) };

    let new_len = a_len + b_len;


    let new_ptr = alloc_array_with_header(new_len as usize, elem_size);
    let new_data_ptr = new_ptr as *mut u8;


    if a_len > 0 {
        std::ptr::copy_nonoverlapping(a_ptr, new_data_ptr, (a_len as usize) * elem_size);
    }


    if b_len > 0 {
        std::ptr::copy_nonoverlapping(b_ptr, new_data_ptr.add((a_len as usize) * elem_size), (b_len as usize) * elem_size);
    }


    new_ptr
}
#[no_mangle]
pub unsafe extern "C" fn quantica_rt_file_append(fname_ptr: *const c_char, content_ptr: *const c_char) {
    if fname_ptr.is_null() || content_ptr.is_null() { return; }

    let fname = CStr::from_ptr(fname_ptr).to_string_lossy();
    let content = CStr::from_ptr(content_ptr).to_string_lossy();


    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(fname.as_ref())
        .unwrap_or_else(|_| panic!("Failed to open file for appending: {}", fname));

    let _ = write!(file, "{}", content);
}