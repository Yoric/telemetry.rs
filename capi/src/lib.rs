#![feature(const_fn)]
#![allow(non_camel_case_types)]

extern crate atomic_cell;
extern crate libc;
extern crate telemetry;

use atomic_cell::{CleanGuard, StaticCell};
use std::ptr;
use std::ffi::{CStr, CString};
use std::str;
use std::sync::Arc;
use std::sync::mpsc::channel;
use telemetry::{Subset, Service};
use telemetry::plain::{Histogram, Flag, Count};

pub struct telemetry_t {
    _guard: CleanGuard<'static>,
    flags: Vec<*mut flag_t>,
    counts: Vec<*mut count_t>,
}

impl telemetry_t {
    fn new(is_active: bool) -> telemetry_t {
        let guard = TELEMETRY.init(Arc::new(Service::new(is_active)));
        telemetry_t {
            flags: vec![],
            counts: vec![],
            _guard: guard,
        }
    }
}

impl Drop for telemetry_t {
    fn drop(&mut self) {
        unsafe {
            for flag in &self.flags {
                free_flag(*flag);
            }

            for count in &self.counts {
                free_count(*count);
            }
        }
    }
}

trait CHistogram {
    fn new(name: &str, telemetry: &Service) -> Self;
}

pub struct flag_t {
    inner: Flag,
}

impl CHistogram for flag_t {
    fn new(name: &str, telemetry: &Service) -> flag_t {
        flag_t {
            inner: Flag::new(&telemetry, name.to_owned())
        }
    }
}

pub struct count_t {
    inner: Count,
}

impl CHistogram for count_t {
    fn new(name: &str, telemetry: &Service) -> count_t {
        count_t {
            inner: Count::new(&telemetry, name.to_owned())
        }
    }
}

unsafe fn new_histogram<T: CHistogram>(name: *const libc::c_char) -> *mut T {
    assert!(!name.is_null());
    let c_str = CStr::from_ptr(name);

    let r_str = match str::from_utf8(c_str.to_bytes()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let telemetry = TELEMETRY.get().unwrap();

    Box::into_raw(Box::new(T::new(r_str, &telemetry)))
}

static TELEMETRY: StaticCell<Arc<Service>> = StaticCell::new();

#[no_mangle]
pub unsafe extern "C" fn telemetry_init(is_active: libc::c_int) -> *mut telemetry_t {
    let is_active = if is_active != 0 { true } else { false };
    Box::into_raw(Box::new(telemetry_t::new(is_active)))
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_free(telemetry: *mut telemetry_t) {
    let telemetry = Box::from_raw(telemetry);
    drop(telemetry);
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_new_flag(name: *const libc::c_char) -> *mut flag_t {
    new_histogram(name)
}

unsafe fn free_flag(flag: *mut flag_t) {
    let flag = Box::from_raw(flag);
    drop(flag);
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_add_flag(telemetry: *mut telemetry_t, flag: *mut flag_t) {
    let telemetry = &mut *telemetry;
    telemetry.flags.push(flag);
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_record_flag(flag: *mut flag_t) {
    (*flag).inner.record(());
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_new_count(name: *const libc::c_char) -> *mut count_t {
    new_histogram(name)
}

unsafe fn free_count(count: *mut count_t) {
    let flag = Box::from_raw(count);
    drop(flag);
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_add_count(telemetry: *mut telemetry_t, count: *mut count_t) {
    let telemetry = &mut *telemetry;
    telemetry.counts.push(count);
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_record_count(count: *mut count_t, value: libc::c_uint) {
    (*count).inner.record(value);
}

fn serialize(subset: Subset) -> Option<String> {
    let service = TELEMETRY.get().unwrap();
    let (sender, receiver) = channel();
    service.to_json(subset, telemetry::SerializationFormat::SimpleJson, sender);
    receiver.recv().ok().map(|s| format!("{}", s.pretty()))
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_serialize_plain_json() -> *mut libc::c_char {
    serialize(Subset::AllPlain)
        .and_then(|s| CString::new(s).ok())
        .map(|s| s.into_raw())
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_free_serialized_json(s: *mut libc::c_char) {
    let s = CString::from_raw(s);
    drop(s);
}

#[test]
fn it_works() {
    unsafe {
        let telemetry = telemetry_init(1);

        let name = CString::new("FLAG").unwrap();
        let flag = telemetry_new_flag(name.as_ptr());
        telemetry_add_flag(telemetry, flag);

        let name = CString::new("COUNT").unwrap();
        let count = telemetry_new_count(name.as_ptr());
        telemetry_add_count(telemetry, count);

        telemetry_record_flag(flag);

        telemetry_record_count(count, 2);
        telemetry_record_count(count, 3);

        let s = telemetry_serialize_plain_json();
        let repr = CStr::from_ptr(s as *const libc::c_char).to_string_lossy();
        assert_eq!(repr, "{\n  \"COUNT\": 5,\n  \"FLAG\": 1\n}");

        telemetry_free_serialized_json(s);
        telemetry_free(telemetry);
    }
}
