#![allow(non_camel_case_types)]

extern crate libc;
extern crate telemetry;

use std::ptr;
use std::ffi::{CStr, CString};
use std::str;
use std::sync::mpsc::channel;
use telemetry::{Subset, Service, SerializationFormat};
use telemetry::plain::{Histogram, Flag, Count};

pub struct telemetry_t {
    service: Service,
    flags: Vec<*mut flag_t>,
    counts: Vec<*mut count_t>,
}

impl telemetry_t {
    fn new(is_active: bool) -> telemetry_t {
        telemetry_t {
            flags: vec![],
            counts: vec![],
            service: Service::new(is_active),
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

unsafe fn new_histogram<T: CHistogram>(telemetry: &Service, name: *const libc::c_char) -> *mut T {
    assert!(!name.is_null());
    let c_str = CStr::from_ptr(name);

    let r_str = match str::from_utf8(c_str.to_bytes()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    Box::into_raw(Box::new(T::new(r_str, &telemetry)))
}

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
pub unsafe extern "C" fn telemetry_new_flag(telemetry: *mut telemetry_t,
                                            name: *const libc::c_char) -> *mut flag_t {
    let flag = new_histogram(&(*telemetry).service, name);
    (*telemetry).flags.push(flag);
    flag
}

unsafe fn free_flag(flag: *mut flag_t) {
    let flag = Box::from_raw(flag);
    drop(flag);
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_record_flag(flag: *mut flag_t) {
    (*flag).inner.record(());
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_new_count(telemetry: *mut telemetry_t,
                                             name: *const libc::c_char) -> *mut count_t {
    let count = new_histogram(&(*telemetry).service, name);
    (*telemetry).counts.push(count);
    count
}

unsafe fn free_count(count: *mut count_t) {
    let flag = Box::from_raw(count);
    drop(flag);
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_record_count(count: *mut count_t, value: libc::c_uint) {
    (*count).inner.record(value);
}

fn serialize(telemetry: &telemetry_t, subset: Subset) -> Option<String> {
    let (sender, receiver) = channel();
    telemetry.service.to_json(subset, SerializationFormat::SimpleJson, sender);
    receiver.recv().ok().map(|s| format!("{}", s.pretty()))
}

#[no_mangle]
pub unsafe extern "C" fn telemetry_serialize_plain_json(telemetry: *mut telemetry_t)
                                                        -> *mut libc::c_char {
    serialize(&*telemetry, Subset::AllPlain)
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
        let flag = telemetry_new_flag(telemetry, name.as_ptr());

        let name = CString::new("COUNT").unwrap();
        let count = telemetry_new_count(telemetry, name.as_ptr());

        telemetry_record_flag(flag);

        telemetry_record_count(count, 2);
        telemetry_record_count(count, 3);

        let s = telemetry_serialize_plain_json(telemetry);
        let repr = CStr::from_ptr(s as *const libc::c_char).to_string_lossy();
        assert_eq!(repr, "{\n  \"COUNT\": 5,\n  \"FLAG\": 1\n}");

        telemetry_free_serialized_json(s);
        telemetry_free(telemetry);
    }
}
