#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_variables, unused_mut)]
#![allow(unused)]

#[macro_use]
pub mod macros; /* Handy macros */

// use std;
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::fmt;
use std::fmt::{Debug, Error, Formatter};
use std::marker::PhantomData;
use std::time::{Duration, Instant};
use uds;

mod vpp {
    pub mod vec {

        #[repr(C)]
        #[derive(Copy, Clone, Debug)]
        struct VppVecHeader {
            #[doc = "Number of elements in vector (NOT its allocated length)."]
            pub len: u32,
            #[doc = "header size divided by VEC_MIN_ALIGN"]
            pub hdr_size: u8,
            #[doc = "log2_align : 7, default_heap : 1"]
            log2_align_7_default_heap_1: u8,
            #[doc = "number of elts vector can grow without realloc"]
            pub grow_elts: u8,
            #[doc = "pad to 8 bytes"]
            pub vpad: [u8; 1usize],
        }

        unsafe fn vec_find(v: *const libc::c_void) -> *const VppVecHeader {
            (v as *const VppVecHeader).offset(-1)
        }

        pub unsafe fn vec_len(v: *const libc::c_void) -> usize {
            (*vec_find(v)).len as usize
        }

        pub unsafe fn vec_elt_at_index<T>(vec: *const T, index: usize) -> *const T {
            vec.offset(index.try_into().unwrap())
        }
    }
}

pub const VLIB_STATS_MAX_NAME_SZ: usize = 128;
#[cfg(feature = "c-client")]
pub mod sys {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
#[cfg(not(feature = "c-client"))]
pub mod sys {
    #[doc = " 64bit counters"]
    pub type counter_t = u64;
    #[doc = " Combined counter to hold both packets and byte differences."]
    #[repr(C)]
    #[derive(Debug, Default, Copy, Clone)]
    pub struct vlib_counter_t {
        #[doc = "< packet counter"]
        pub packets: counter_t,
        #[doc = "< byte counter"]
        pub bytes: counter_t,
    }
    pub const STAT_COUNTER_HEARTBEAT: usize = 0;

    pub const STAT_DIR_TYPE_ILLEGAL: stat_directory_type_t = 0;
    pub const STAT_DIR_TYPE_SCALAR_INDEX: stat_directory_type_t = 1;
    pub const STAT_DIR_TYPE_COUNTER_VECTOR_SIMPLE: stat_directory_type_t = 2;
    pub const STAT_DIR_TYPE_COUNTER_VECTOR_COMBINED: stat_directory_type_t = 3;
    pub const STAT_DIR_TYPE_NAME_VECTOR: stat_directory_type_t = 4;
    pub const STAT_DIR_TYPE_EMPTY: stat_directory_type_t = 5;
    pub const STAT_DIR_TYPE_SYMLINK: stat_directory_type_t = 6;
    pub type stat_directory_type_t = ::std::os::raw::c_uint;

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    pub struct vlib_stats_entry_t {}
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub union vlib_stats_entry_t__bindgen_ty_1 {
        pub x2u32: u32x2_struct_t,
        pub index: u64,
        pub value: u64,
        pub data: *mut ::std::os::raw::c_void,
        pub string_vector: *mut *mut u8,
    }
    #[repr(C)]
    #[derive(Debug, Default, Copy, Clone)]
    pub struct u32x2_struct_t {
        pub index1: u32,
        pub index2: u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct stat_segment_data_t {
        pub name: *mut ::std::os::raw::c_char,
        pub type_: stat_directory_type_t,
        pub via_symlink: bool,
        pub __bindgen_anon_1: stat_segment_data_t__bindgen_ty_1,
    }
    #[repr(C)]
    #[derive(Copy, Clone)]
    pub union stat_segment_data_t__bindgen_ty_1 {
        pub scalar_value: ::std::os::raw::c_double,
        pub error_vector: *mut counter_t,
        pub simple_counter_vec: *mut *mut counter_t,
        pub combined_counter_vec: *mut *mut vlib_counter_t,
        pub name_vector: *mut *mut u8,
    }

    use crate::VlibStatsEntry;
    use crate::VlibStatsSharedHeader;
    use crate::VppStatClient;
    use std::cell::RefCell;
    use std::time::{Duration, Instant};

    pub struct StatSegmentAccess {
        /* shared header from the client */
        shared_header: *const crate::VlibStatsSharedHeader,
        directory_vector: *const VlibStatsEntry,
        epoch: u64,
    }

    impl StatSegmentAccess {
        pub fn start(client: &VppStatClient) -> Option<Self> {
            let shared_header = client.get_shared_header();
            let maybe_timeout = client.get_timeout();
            let epoch = unsafe { std::ptr::read_volatile(shared_header).epoch };
            let success = if let Some(timeout) = maybe_timeout {
                let deadline = Instant::now().checked_add(timeout).unwrap();
                while unsafe { std::ptr::read_volatile(shared_header).in_progress } != 0
                    && Instant::now() < deadline
                { /* busy loop */ }
                Instant::now() < deadline
            } else {
                while unsafe { std::ptr::read_volatile(shared_header).in_progress } != 0 {
                    /* busy loop */
                }
                true
            };
            let directory_vector = unsafe {
                Self::stat_segment_adjust_x(shared_header, unsafe {
                    std::ptr::read_volatile(shared_header).directory_vector
                })
                .unwrap()
            };
            *client.main.directory_vector.borrow_mut() = directory_vector;
            if success {
                Some(Self {
                    shared_header,
                    directory_vector,
                    epoch,
                })
            } else {
                None
            }
        }
        #[cfg(not(feature = "c-client"))]
        pub fn stat_segment_adjust_x<T>(
            shared_header: *const VlibStatsSharedHeader,
            ptr: *const T,
        ) -> Option<*const T> {
            unsafe {
                Some((shared_header as *const u8).offset(
                    (ptr as *const u8).offset_from(std::ptr::read_volatile(shared_header).base),
                ) as *const T)
            }
        }

        #[cfg(not(feature = "c-client"))]
        pub fn stat_segment_adjust<T>(&self, ptr: *const T) -> Option<*const T> {
            let shared_header = self.shared_header;
            /* the mapping in the original process and mapping in the current process
             * will have different logical memory addresses. Adjust for that */
            Self::stat_segment_adjust_x(shared_header, ptr)
        }
        #[cfg(not(feature = "c-client"))]
        pub fn get_stat_vector_r(&self) -> Option<*const VlibStatsEntry> {
            let shared_header = self.shared_header;
            unsafe {
                self.stat_segment_adjust(std::ptr::read_volatile(shared_header).directory_vector)
            }
        }

        pub fn get_epoch(&self) -> u64 {
            self.epoch
        }
        pub fn get_directory_vector(&self) -> *const VlibStatsEntry {
            self.directory_vector
        }
        pub fn data_changed(&self) -> bool {
            let epoch = unsafe { std::ptr::read_volatile(self.shared_header).epoch };
            let in_progress = unsafe { std::ptr::read_volatile(self.shared_header).in_progress };
            self.epoch != epoch || in_progress != 0
        }
        pub fn end(self) -> Result<(), crate::VppStatSegmentAccessError> {
            use crate::VppStatSegmentAccessError;
            if self.data_changed() {
                Err(VppStatSegmentAccessError::StatSegmentChanged)
            } else {
                Ok(())
            }
        }
    }
}

macro_rules! cstr {
    ($s:expr) => {
        concat!($s, "\0") as *const str as *const [i8] as *const i8
    };
}

macro_rules! cstr_mut {
    ($s:expr) => {
        concat!($s, "\0") as *const str as *mut [i8] as *mut i8
    };
}

macro_rules! ucstr {
    ($s:expr) => {
        concat!($s, "\0") as *const str as *const [u8] as *const u8
    };
}

macro_rules! ucstr_mut {
    ($s:expr) => {
        concat!($s, "\0") as *const str as *mut [u8] as *mut u8
    };
}

use libc::c_char;
use std::ffi::CStr;
use std::ops::Index;
use std::str;

fn ptr2str(cstrptr: *const c_char) -> &'static str {
    let c_str: &CStr = unsafe { CStr::from_ptr(cstrptr) };
    let str_slice: &str = c_str.to_str().unwrap();
    str_slice
}

fn vv2slice<T>(vv: *const T) -> &'static [T] {
    unsafe {
        #[cfg(feature = "c-client")]
        let vv_len = sys::stat_segment_vec_len(vv as *mut libc::c_void) as usize;
        #[cfg(not(feature = "c-client"))]
        let vv_len = vpp::vec::vec_len(vv as *mut libc::c_void) as usize;
        let slice: &[T] = core::slice::from_raw_parts(vv, vv_len);
        slice
    }
}
// use crate::sys::*;

struct CounterCombined {
    packets: u64,
    bytes: u64,
}

pub struct DataVecVec<'a, T> {
    #[cfg(feature = "c-client")]
    vector_ptr: &'a [*mut T],
    #[cfg(not(feature = "c-client"))]
    vector: Vec<Vec<T>>,
    #[cfg(not(feature = "c-client"))]
    _phantom: PhantomData<&'a T>,
}

pub struct NameVec<'a> {
    vector_ptr: &'a [*mut u8],
}

use std::slice::SliceIndex;

impl<'a, T> DataVecVec<'a, T> {
    pub fn len(&self) -> usize {
        #[cfg(feature = "c-client")]
        let x = self.vector_ptr.len();
        #[cfg(not(feature = "c-client"))]
        let x = self.vector.len();
        x
    }
}

impl<'a, T> Index<usize> for DataVecVec<'a, T> {
    type Output = [T];

    fn index(&self, index: usize) -> &Self::Output {
        #[cfg(feature = "c-client")]
        unsafe {
            let vv = self.vector_ptr[index];
            let vv_len = sys::stat_segment_vec_len(vv as *mut libc::c_void) as usize;
            let slice: &[T] = core::slice::from_raw_parts(vv, vv_len);
            slice
        }
        #[cfg(not(feature = "c-client"))]
        {
            let slice: &[T] = &self.vector[index];
            slice
        }
    }
}

impl<'a, T: std::fmt::Debug> fmt::Debug for DataVecVec<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.len() {
            write!(f, "{:?}", &self[i]);
        }
        Ok(())
    }
}

impl<'a> NameVec<'a> {
    pub fn len(&self) -> usize {
        self.vector_ptr.len()
    }
}

impl<'a> Index<usize> for NameVec<'a> {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            let vv = self.vector_ptr[index];
            let c_str: &CStr = unsafe { CStr::from_ptr(vv as *const c_char) };
            let slice: &str = c_str.to_str().unwrap();
            slice
        }
    }
}

impl<'a> fmt::Debug for NameVec<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.len() {
            write!(f, "{:?} ", &self[i]);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum StatValue<'a> {
    Illegal,
    ScalarIndex(f64),
    CounterVectorSimple(DataVecVec<'a, u64>),
    CounterVectorCombined(DataVecVec<'a, sys::vlib_counter_t>),
    NameVector(NameVec<'a>),
    Empty,
    Symlink,
}

pub struct StatSegmentData<'a> {
    #[cfg(feature = "c-client")]
    orig_data: &'a sys::stat_segment_data_t,
    pub name: &'a str,
    pub value: StatValue<'a>,
    pub via_symlink: bool,
}
impl<'a> fmt::Debug for StatSegmentData<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.name, self.value)
    }
}

impl<'a> StatSegmentData<'a> {
    #[cfg(not(feature = "c-client"))]
    fn from_ctype(access: &crate::sys::StatSegmentAccess, item: &'a VlibStatsEntry) -> Self {
        Self::copy_data(access, item, None, None, false)
    }

    #[cfg(not(feature = "c-client"))]
    fn copy_data(
        access: &crate::sys::StatSegmentAccess,
        item: &'a VlibStatsEntry,
        index2: Option<usize>,
        maybe_name: Option<*const u8>,
        via_symlink: bool,
    ) -> Self {
        use crate::sys::vlib_counter_t;
        let c_str: &CStr = unsafe { CStr::from_ptr(maybe_name.unwrap_or(&item.name as *const u8)) };
        let name: &str = c_str.to_str().unwrap();
        println!(
            "Name: {} type: {}, via_symlink: {}",
            &name, item.type_, via_symlink
        );
        let value = match item.type_ {
            sys::STAT_DIR_TYPE_ILLEGAL => StatValue::Illegal,
            sys::STAT_DIR_TYPE_SCALAR_INDEX => {
                let val = unsafe { item.__bindgen_anon_1.value };
                StatValue::ScalarIndex(val as f64)
            }
            sys::STAT_DIR_TYPE_SYMLINK => {
                let dir_index = unsafe { item.__bindgen_anon_1.x2u32.index1 };
                let target_index = unsafe { item.__bindgen_anon_1.x2u32.index2 };
                let dslice = vv2slice(access.get_directory_vector());
                let entry = &dslice[dir_index as usize];

                let c_str: &CStr = unsafe { CStr::from_ptr(&entry.name as *const u8) };
                let name: &str = c_str.to_str().unwrap();
                println!("link target Name: {}", &name);
                println!(
                    "symlink with dir index {} target index {}",
                    dir_index, target_index
                );
                return Self::copy_data(
                    access,
                    entry,
                    Some(target_index as usize),
                    Some(&item.name as *const u8),
                    true,
                );
            }
            sys::STAT_DIR_TYPE_COUNTER_VECTOR_SIMPLE => {
                let simple_c: *const *const u64 = unsafe {
                    access
                        .stat_segment_adjust(item.__bindgen_anon_1.data)
                        .unwrap() as *const *const u64
                };
                let slice_simple_c = vv2slice(simple_c);
                let mut vvec: Vec<Vec<u64>> = vec![];

                for (i, x) in slice_simple_c.into_iter().enumerate() {
                    let mut counters: Vec<u64> = vec![];
                    let cvec = unsafe { access.stat_segment_adjust(*x).unwrap() };
                    let s_cvec = vv2slice(cvec);
                    if let Some(idx2) = index2 {
                        counters.push(s_cvec[idx2]);
                    } else {
                        for c in s_cvec {
                            counters.push(*c);
                        }
                    }
                    vvec.push(counters);
                }
                // let () = item; // VlibStatsEntry
                // let vs = vv2slice(unsafe { item.__bindgen_anon_1.__bindgen_anon_1.simple_counter_vec });
                // let val = DataVecVec { vector_ptr: vs };
                let val = DataVecVec {
                    vector: vvec,
                    _phantom: PhantomData,
                };
                StatValue::CounterVectorSimple(val)
            }
            sys::STAT_DIR_TYPE_COUNTER_VECTOR_COMBINED => {
                let simple_c: *const *const vlib_counter_t = unsafe {
                    access
                        .stat_segment_adjust(item.__bindgen_anon_1.data)
                        .unwrap() as *const *const vlib_counter_t
                };
                let slice_simple_c = vv2slice(simple_c);
                let mut vvec: Vec<Vec<vlib_counter_t>> = vec![];

                for (i, x) in slice_simple_c.into_iter().enumerate() {
                    let mut counters: Vec<vlib_counter_t> = vec![];
                    let cvec = unsafe { access.stat_segment_adjust(*x).unwrap() };
                    let s_cvec = vv2slice(cvec);
                    if let Some(idx2) = index2 {
                        counters.push(s_cvec[idx2]);
                    } else {
                        for c in s_cvec {
                            counters.push(*c);
                        }
                    }
                    vvec.push(counters);
                }
                // let () = item; // VlibStatsEntry
                // let vs = vv2slice(unsafe { item.__bindgen_anon_1.__bindgen_anon_1.simple_counter_vec });
                // let val = DataVecVec { vector_ptr: vs };
                let val = DataVecVec {
                    vector: vvec,
                    _phantom: PhantomData,
                };
                StatValue::CounterVectorCombined(val)
            }
            x => unimplemented!("Unimplemented entry type {}", x),
        };
        StatSegmentData {
            name,
            value,
            via_symlink,
        }
    }
    #[cfg(feature = "c-client")]
    fn from_ctype(item: &'a sys::stat_segment_data_t) -> Self {
        let c_str: &CStr = unsafe { CStr::from_ptr(item.name) };
        let name: &str = c_str.to_str().unwrap();

        let value = match item.type_ {
            sys::STAT_DIR_TYPE_ILLEGAL => StatValue::Illegal,
            sys::STAT_DIR_TYPE_SCALAR_INDEX => {
                let val = unsafe { item.__bindgen_anon_1.scalar_value };
                StatValue::ScalarIndex(val)
            }
            sys::STAT_DIR_TYPE_COUNTER_VECTOR_SIMPLE => {
                let vs = vv2slice(unsafe { item.__bindgen_anon_1.simple_counter_vec });
                let val = DataVecVec { vector_ptr: vs };
                StatValue::CounterVectorSimple(val)
            }
            sys::STAT_DIR_TYPE_COUNTER_VECTOR_COMBINED => {
                let vc = vv2slice(unsafe { item.__bindgen_anon_1.combined_counter_vec });
                let val = DataVecVec { vector_ptr: vc };
                StatValue::CounterVectorCombined(val)
            }
            sys::STAT_DIR_TYPE_NAME_VECTOR => {
                let nv = vv2slice(unsafe { item.__bindgen_anon_1.name_vector });
                let val = NameVec { vector_ptr: nv };
                StatValue::NameVector(val)
            }
            sys::STAT_DIR_TYPE_EMPTY => StatValue::Empty,
            sys::STAT_DIR_TYPE_SYMLINK => StatValue::Symlink,
            7_u32..=u32::MAX => unimplemented!(),
        };

        StatSegmentData {
            orig_data: item,
            name,
            value,
            via_symlink: item.via_symlink,
        }
    }
}

/*
use std::arch::asm;

#[cfg(any(target_arch = "arm64", target_arch = "aarch64"))]
pub fn rdtscp() -> u64 {
    let mut val: u64;
    unsafe {
        asm!(
            "mrs {0}, cntvct_el0",
            out(reg) val,
            options(nostack)
        );
    }

    val
}


/* https://lukas-prokop.at/articles/2021-11-10-rdtsc-with-rust-asm */
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn rdtscp() -> u64 {
    let eax: u32;
    let ecx: u32;
    let edx: u32;
    {
        unsafe {
            asm!(
              "rdtscp",
              lateout("eax") eax,
              lateout("ecx") ecx,
              lateout("edx") edx,
              options(nomem, nostack)
            );
        }
    }

    let counter: u64 = (edx as u64) << 32 | eax as u64;
    counter
}

#[no_mangle]
fn bench<F>(f: F) -> u64
where
    F: Fn(),
{
    let pre = rdtscp(); // unsafe { core::arch::x86_64::_rdtsc() };
    f();
    let post = rdtscp(); // unsafe { core::arch::x86_64::_rdtsc() };
    post - pre
}
*/

#[repr(C)]
pub struct VlibStatsEntry {
    pub type_: sys::stat_directory_type_t,
    pub __bindgen_anon_1: sys::vlib_stats_entry_t__bindgen_ty_1,
    pub name: [::std::os::raw::c_char; VLIB_STATS_MAX_NAME_SZ],
}

#[repr(C)]
pub struct VlibStatsSharedHeader {
    version: u64,
    base: *const u8,
    epoch: u64,                              /* volatile */
    in_progress: u64,                        /* volatile */
    directory_vector: *const VlibStatsEntry, /* volatile */
}

#[repr(C)]
#[derive(Debug)]
pub struct StatClientMain {
    heartbeat_epoch: RefCell<u64>,
    shared_header: *const VlibStatsSharedHeader,
    directory_vector: RefCell<*const VlibStatsEntry>,
    memory_size: usize,
    timeout: Option<Duration>,
}

/* sanity checks - to ensure the structures make sense */
#[cfg(feature = "c-client")]
mod size_checks {
    use crate::sys::*;
    use crate::*;
    /*
     * This is no longer true, now that timeout is an option instead of u64!
     *
     * const TestChecker1: [u8; std::mem::size_of::<StatClientMain>()] =
     *    [0; std::mem::size_of::<sys::stat_client_main_t>()];
     */
    const TestChecker2: [u8; std::mem::size_of::<VlibStatsSharedHeader>()] =
        [0; std::mem::size_of::<sys::vlib_stats_shared_header_t>()];
    const TestChecker3: [u8; std::mem::size_of::<VlibStatsEntry>()] =
        [0; std::mem::size_of::<sys::vlib_stats_entry_t>()];
}

#[derive(Debug)]
pub struct VppStatClient {
    #[cfg(feature = "c-client")]
    stat_client_ptr: *mut sys::stat_client_main_t,
    #[cfg(not(feature = "c-client"))]
    main: StatClientMain,
    #[cfg(not(feature = "c-client"))]
    mmap: memmap::Mmap,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VppStatSegmentAccessError {
    StatSegmentChanged,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VppStatLsError {
    RegexCompileError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VppStatError {
    CouldNotOpenSocket,
    CouldNotConnect,
    ReceivingFdFailed,
    MmapFstatFailed,
    MmapMapFailed,
}

pub struct VppStringVec {
    #[cfg(feature = "c-client")]
    vvec_ptr: *mut *mut u8,
    #[cfg(not(feature = "c-client"))]
    strings: Vec<String>,
}

impl VppStringVec {
    #[cfg(feature = "c-client")]
    pub fn new() -> Self {
        let vvec_ptr = std::ptr::null_mut();
        VppStringVec { vvec_ptr }
    }

    #[cfg(not(feature = "c-client"))]
    pub fn new() -> Self {
        VppStringVec { strings: vec![] }
    }

    #[cfg(feature = "c-client")]
    pub fn push(&mut self, s: &str) {
        let cs = format!("{}\0", s);
        let cstr_ptr = cs.as_str() as *const str as *const [i8] as *const c_char;
        self.vvec_ptr = unsafe { sys::stat_segment_string_vector(self.vvec_ptr, cstr_ptr) };
    }
    #[cfg(not(feature = "c-client"))]
    pub fn push(&mut self, s: &str) {
        self.strings.push(s.to_string());
    }

    #[cfg(feature = "c-client")]
    pub fn len(&self) -> usize {
        let vv_len =
            unsafe { sys::stat_segment_vec_len(self.vvec_ptr as *mut libc::c_void) as usize };
        vv_len
    }
    #[cfg(not(feature = "c-client"))]
    pub fn len(&self) -> usize {
        self.strings.len()
    }
}

impl Index<usize> for VppStringVec {
    type Output = str;

    #[cfg(feature = "c-client")]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            let vv_len = sys::stat_segment_vec_len(self.vvec_ptr as *mut libc::c_void) as usize;
            let slice: &[*mut u8] = core::slice::from_raw_parts(self.vvec_ptr, vv_len);

            let vv = slice[index];
            let c_str: &CStr = unsafe { CStr::from_ptr(vv as *const c_char) };
            let slice: &str = c_str.to_str().unwrap();
            slice
        }
    }

    #[cfg(not(feature = "c-client"))]
    fn index(&self, index: usize) -> &Self::Output {
        &self.strings[index]
    }
}

impl fmt::Debug for VppStringVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.len() {
            write!(f, "{:?} ", &self[i]);
        }
        Ok(())
    }
}

#[cfg(feature = "c-client")]
impl Drop for VppStringVec {
    fn drop(&mut self) {
        unsafe {
            let vv_len = sys::stat_segment_vec_len(self.vvec_ptr as *mut libc::c_void) as usize;
            let slice: &[*mut u8] = core::slice::from_raw_parts(self.vvec_ptr, vv_len);
            for i in 0..slice.len() {
                let vv = slice[i];
                sys::stat_segment_vec_free(vv as *mut libc::c_void);
            }
            sys::stat_segment_vec_free(self.vvec_ptr as *mut libc::c_void);
        };
    }
}

pub struct VppStatDir<'a> {
    client: &'a VppStatClient,
    #[cfg(feature = "c-client")]
    dir_ptr: *const u32,
    #[cfg(feature = "c-client")]
    dir: &'a [u32],

    #[cfg(not(feature = "c-client"))]
    dir_vec: Vec<u32>,
    #[cfg(not(feature = "c-client"))]
    current_epoch: u64,
}

#[cfg(feature = "c-client")]
impl Drop for VppStatDir<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::stat_segment_vec_free(self.dir_ptr as *mut libc::c_void);
        };
    }
}

pub struct VppStatData<'a> {
    #[cfg(feature = "c-client")]
    stat_client_ptr: *mut sys::stat_client_main_t,
    #[cfg(feature = "c-client")]
    data_ptr: *const sys::stat_segment_data_t,
    #[cfg(feature = "c-client")]
    data: &'a [sys::stat_segment_data_t],
    #[cfg(not(feature = "c-client"))]
    stat_data: &'a u32,
}

pub struct VppStatDataIterator<'a> {
    // #[cfg(feature = "c-client")]
    stat_data: &'a VppStatData<'a>,
    #[cfg(feature = "c-client")]
    curr: usize,
}

impl<'a> Iterator for VppStatDataIterator<'a> {
    type Item = StatSegmentData<'a>;
    #[cfg(not(feature = "c-client"))]
    fn next(&mut self) -> Option<Self::Item> {
        panic!("fixme");
        None
    }
    #[cfg(feature = "c-client")]
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.stat_data.len() {
            let curr = self.curr;
            self.curr = curr + 1;
            let cptr = self.stat_data.stat_client_ptr;
            Some(StatSegmentData::from_ctype(&self.stat_data.data[curr]))
        } else {
            None
        }
    }
}

impl<'a> VppStatData<'a> {
    #[cfg(feature = "c-client")]
    pub fn iter(&'a self) -> VppStatDataIterator {
        VppStatDataIterator {
            stat_data: self,
            curr: 0,
        }
    }
    #[cfg(not(feature = "c-client"))]
    pub fn iter(&'a self) -> VppStatDataIterator {
        VppStatDataIterator { stat_data: self }
    }
    #[cfg(feature = "c-client")]
    pub fn len(&self) -> usize {
        self.data.len()
    }
    #[cfg(not(feature = "c-client"))]
    pub fn len(&self) -> usize {
        panic!("FIXME")
    }
    #[cfg(feature = "c-client")]
    pub fn is_empty(&self) -> bool {
        self.data.len() == 0
    }
    #[cfg(not(feature = "c-client"))]
    pub fn is_empty(&self) -> bool {
        panic!("FIXME")
    }
}

#[cfg(feature = "c-client")]
impl Drop for VppStatData<'_> {
    fn drop(&mut self) {
        unsafe { sys::stat_segment_data_free(self.data_ptr as *mut sys::stat_segment_data_t) };
    }
}

pub struct VppStatDirNamesIterator<'a> {
    dir: &'a VppStatDir<'a>,
    curr: usize,
}

impl<'a> Iterator for VppStatDirNamesIterator<'a> {
    type Item = String;

    #[cfg(not(feature = "c-client"))]
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.dir.dir_vec.len() {
            let curr = self.curr;
            self.curr = curr + 1;
            self.dir
                .client
                .stat_segment_index_to_name_r(self.dir.dir_vec[curr])
        } else {
            None
        }
    }

    #[cfg(feature = "c-client")]
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.dir.dir.len() {
            let curr = self.curr;
            self.curr = curr + 1;
            let name = unsafe {
                sys::stat_segment_index_to_name_r(
                    self.dir.dir[curr],
                    self.dir.client.stat_client_ptr,
                )
            };
            let out = ptr2str(name).to_string();
            unsafe {
                libc::free(name as *mut libc::c_void);
            }
            Some(out)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VppStatDumpError {
    ObsoleteDirData,
    AccessStartFailed,
}

impl<'a, 'b: 'a> VppStatDir<'a> {
    #[cfg(not(feature = "c-client"))]
    pub fn dump(&'a self) -> Result<VppStatData<'b>, VppStatDumpError> {
        use crate::VppStatDumpError::*;
        let mut out: Vec<StatSegmentData<'b>> = vec![];

        let access = sys::StatSegmentAccess::start(&self.client);
        if access.is_none() {
            return Err(AccessStartFailed);
        }
        let access = access.unwrap();
        if access.get_epoch() != self.current_epoch {
            return Err(ObsoleteDirData);
        }

        let counter_vec = access.get_stat_vector_r().unwrap();
        let counter_slice = vv2slice(counter_vec);

        for (i, index) in self.dir_vec.iter().enumerate() {
            let v = &counter_slice[*index as usize];

            let newval = StatSegmentData::from_ctype(&access, v);
            println!("{}: {:?}", index, &newval);
        }

        panic!("fixme")
    }
    #[cfg(feature = "c-client")]
    pub fn dump(&'a self) -> Result<VppStatData<'b>, VppStatDumpError> {
        use crate::VppStatDumpError::ObsoleteDirData;
        let res = unsafe {
            sys::stat_segment_dump_r(self.dir_ptr as *mut u32, self.client.stat_client_ptr)
        };
        if res == std::ptr::null_mut() {
            return Err(ObsoleteDirData);
        }
        let res_len = unsafe { sys::stat_segment_vec_len(res as *mut libc::c_void) as usize };
        let slice: &'b [sys::stat_segment_data_t] =
            unsafe { core::slice::from_raw_parts(res, res_len) };
        Ok(VppStatData {
            // dir: self,
            stat_client_ptr: self.client.stat_client_ptr,
            data_ptr: res,
            data: slice,
        })
    }
    pub fn names(&'a self) -> VppStatDirNamesIterator<'a> {
        VppStatDirNamesIterator { dir: self, curr: 0 }
    }
}

impl VppStatClient {
    /* This will likely change - it is not a good ergonomics to require to call this */
    #[cfg(feature = "c-client")]
    pub fn init_once(memsize: Option<usize>) {
        let memsize = if let Some(mem) = memsize {
            mem
        } else {
            64000000
        };
        unsafe {
            sys::clib_mem_init(std::ptr::null_mut(), 64000000);
        }
    }

    /*
               let secs = self.main.timeout / 1000000000u64;
               let usecs = (self.main.timeout - secs) as u32;
    * */

    #[cfg(not(feature = "c-client"))]
    pub fn get_timeout(&self) -> Option<Duration> {
        self.main.timeout
    }

    #[cfg(not(feature = "c-client"))]
    pub fn get_shared_header(&self) -> *const VlibStatsSharedHeader {
        self.main.shared_header
    }

    #[cfg(not(feature = "c-client"))]
    pub fn new(mmap: memmap::Mmap) -> Self {
        let memory_size = mmap.len();
        let shared_header_ptr = mmap.as_ptr();
        let raw_ptr: *const u8 = shared_header_ptr;
        let shared_header = unsafe { raw_ptr as *const VlibStatsSharedHeader };
        let main = StatClientMain {
            timeout: None,
            memory_size,
            shared_header,
            directory_vector: RefCell::new(std::ptr::null()),
            heartbeat_epoch: RefCell::new(0),
        };
        VppStatClient { main, mmap }
    }

    #[cfg(not(feature = "c-client"))]
    pub fn connect(path: &str) -> Result<Self, VppStatError> {
        use crate::VppStatError::*;
        use std::fs::File;
        /*
         * 'use std::os::fd::FromRawFd;
        use std::os::fd::RawFd; */
        use std::os::unix::io::FromRawFd;
        use std::os::unix::prelude::RawFd;
        use uds::UnixSeqpacketConn;

        use memmap::Mmap;

        let socket = match UnixSeqpacketConn::connect(path) {
            Ok(sock) => {
                let mut bytes: [u8; 1] = [0; 1];
                let mut fds: [RawFd; 1] = [-1; 1];
                let x = sock.recv_fds(&mut bytes, &mut fds);
                if x.is_err() {
                    return Err(CouldNotOpenSocket);
                }
                let x = x.unwrap();
                let nfds = x.2;
                if nfds < 1 {
                    return Err(ReceivingFdFailed);
                }
                let rawfd = fds[0];
                let mut file = unsafe { File::from_raw_fd(rawfd) };
                let mmap = unsafe { Mmap::map(&file).unwrap() };
                // let mut_mmap = mmap.make_mut().unwrap();
                let len = mmap.len();
                let piece = &mmap[0..128];
                println!("mmap len: {piece:x?}");
                println!("Result: {x:?}, {fds:?}");

                let mut client = Self::new(mmap);
                let access = sys::StatSegmentAccess::start(&client).unwrap();
                client.main.heartbeat_epoch = RefCell::new(access.get_epoch());
                access.end().unwrap();
                return Ok(client);
            }
            Err(e) => {
                println!("Couldn't connect {path}: {e:?}");
                return Err(CouldNotConnect);
            }
        };
    }

    #[cfg(feature = "c-client")]
    pub fn connect(path: &str) -> Result<Self, VppStatError> {
        use crate::VppStatError::*;
        use memmap::Mmap;
        use std::fs::File;
        use sys::*;

        static START: std::sync::Once = std::sync::Once::new();

        START.call_once(|| {
            VppStatClient::init_once(None);
        });

        let sc = unsafe { stat_client_get() };
        let cpath = format!("{}\0", path);
        let cstrpath = cpath.as_str() as *const str as *const [i8] as *const c_char;
        let rv = unsafe { stat_segment_connect_r(cstrpath, sc) };
        match rv {
            0 => Ok(VppStatClient {
                stat_client_ptr: sc,
            }),
            -1 => Err(CouldNotOpenSocket),
            -2 => Err(CouldNotConnect),
            -3 => Err(ReceivingFdFailed),
            -4 => Err(MmapFstatFailed),
            -5 => Err(MmapMapFailed),
            _ => unimplemented!(),
        }
    }

    #[cfg(not(feature = "c-client"))]
    pub fn stat_segment_index_to_name_r(&self, index: u32) -> Option<String> {
        let access = sys::StatSegmentAccess::start(&self).unwrap();
        let counter_vec = access.get_stat_vector_r().unwrap();
        let counter_slice = vv2slice(counter_vec);
        let v = &counter_slice[index as usize];
        let mut out = None;
        if v.type_ != sys::STAT_DIR_TYPE_EMPTY {
            let name = ptr2str(&v.name as *const u8);
            out = Some(name.to_string());
        }
        access.end().unwrap();
        out
    }

    #[cfg(feature = "c-client")]
    pub fn heartbeat(&self) -> f64 {
        unsafe { sys::stat_segment_heartbeat_r(self.stat_client_ptr) }
    }
    #[cfg(not(feature = "c-client"))]
    pub fn heartbeat(&self) -> f64 {
        let shared_header = self.main.shared_header;
        let epoch = unsafe { std::ptr::read_volatile(shared_header).epoch };
        if epoch != *self.main.heartbeat_epoch.borrow() {
            return 0.0;
        }
        let access = sys::StatSegmentAccess::start(&self).unwrap();
        let ep = unsafe {
            vpp::vec::vec_elt_at_index(
                *self.main.directory_vector.borrow(),
                sys::STAT_COUNTER_HEARTBEAT,
            )
        };

        access.end().unwrap();
        unsafe { (*ep).__bindgen_anon_1.value as f64 }
    }

    #[cfg(not(feature = "c-client"))]
    pub fn ls(&self, patterns: Option<&VppStringVec>) -> VppStatDir {
        if let Ok(d) = self.ls_r(patterns) {
            d
        } else {
            VppStatDir {
                client: self,
                current_epoch: 0,
                dir_vec: vec![],
            }
        }
    }

    #[cfg(not(feature = "c-client"))]
    pub fn ls_r(&self, patterns: Option<&VppStringVec>) -> Result<VppStatDir, VppStatLsError> {
        use regex::Regex;
        let mut dir_vec: Vec<u32> = vec![];
        let mut regexes: Vec<Regex> = vec![];

        if let Some(pat_vector) = patterns {
            for pat in &pat_vector.strings {
                match Regex::new(&pat) {
                    Ok(re) => regexes.push(re),
                    Err(e) => {
                        return Err(VppStatLsError::RegexCompileError);
                    }
                }
            }
        }

        let access = sys::StatSegmentAccess::start(&self).unwrap();
        let counter_vec = access.get_stat_vector_r().unwrap();
        let counter_slice = vv2slice(counter_vec);

        for (i, v) in counter_slice.iter().enumerate() {
            let name = ptr2str(&v.name as *const u8); // String::from_utf8(v.name.to_vec()).unwrap();
            let value = unsafe { v.__bindgen_anon_1.value };
            let type_ = v.type_;

            if patterns.is_none() {
                dir_vec.push(i as u32);
            } else {
                for regex in &regexes {
                    if regex.is_match(name) {
                        dir_vec.push(i as u32);
                    }
                }
            }
        }
        let current_epoch = access.get_epoch();

        if access.end().is_ok() {
            *self.main.heartbeat_epoch.borrow_mut() = current_epoch;
        }

        Ok(VppStatDir {
            client: self,
            dir_vec,
            current_epoch,
        })
    }

    #[cfg(feature = "c-client")]
    pub fn ls(&self, patterns: Option<&VppStringVec>) -> VppStatDir {
        let patterns = if let Some(v) = patterns {
            v.vvec_ptr
        } else {
            std::ptr::null_mut()
        };
        let dir_ptr = unsafe { sys::stat_segment_ls_r(patterns, self.stat_client_ptr) };
        let dir = vv2slice(dir_ptr);
        VppStatDir {
            client: &self,
            dir_ptr,
            dir,
        } // FIXME: errors
    }
}

#[cfg(feature = "c-client")]
impl Drop for VppStatClient {
    fn drop(&mut self) {
        unsafe {
            sys::stat_segment_disconnect_r(self.stat_client_ptr);
            sys::stat_client_free(self.stat_client_ptr);
        }
    }
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
