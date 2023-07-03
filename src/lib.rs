#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_variables, unused_mut)]
#![allow(unused)]

#[macro_use]
pub mod macros; /* Handy macros */

// use std;
use std::fmt;
use std::fmt::{Debug, Error, Formatter};
use uds;

pub mod sys {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
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
        let vv_len = sys::stat_segment_vec_len(vv as *mut libc::c_void) as usize;
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
    vector_ptr: &'a [*mut T],
}

pub struct NameVec<'a> {
    vector_ptr: &'a [*mut u8],
}

use std::slice::SliceIndex;

impl<'a, T> DataVecVec<'a, T> {
    pub fn len(&self) -> usize {
        self.vector_ptr.len()
    }
}

impl<'a, T> Index<usize> for DataVecVec<'a, T> {
    type Output = [T];

    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            let vv = self.vector_ptr[index];
            let vv_len = sys::stat_segment_vec_len(vv as *mut libc::c_void) as usize;
            let slice: &[T] = core::slice::from_raw_parts(vv, vv_len);
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
    orig_data: &'a sys::stat_segment_data_t,
    pub name: &'a str,
    pub value: StatValue<'a>,
}
impl<'a> fmt::Debug for StatSegmentData<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.name, self.value)
    }
}

impl<'a> StatSegmentData<'a> {
    fn from_ctype(sc: *mut sys::stat_client_main_t, item: &'a sys::stat_segment_data_t) -> Self {
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
        }
    }
}

/*
use std::arch::asm;

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

const VLIB_STATS_MAX_NAME_SZ: usize = 128;

pub const STAT_DIR_TYPE_ILLEGAL: stat_directory_type_t = 0;
pub const STAT_DIR_TYPE_SCALAR_INDEX: stat_directory_type_t = 1;
pub const STAT_DIR_TYPE_COUNTER_VECTOR_SIMPLE: stat_directory_type_t = 2;
pub const STAT_DIR_TYPE_COUNTER_VECTOR_COMBINED: stat_directory_type_t = 3;
pub const STAT_DIR_TYPE_NAME_VECTOR: stat_directory_type_t = 4;
pub const STAT_DIR_TYPE_EMPTY: stat_directory_type_t = 5;
pub const STAT_DIR_TYPE_SYMLINK: stat_directory_type_t = 6;
pub type stat_directory_type_t = ::std::os::raw::c_uint;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct vlib_stats_entry_t {}
#[repr(C)]
#[derive(Copy, Clone)]
pub union vlib_stats_entry_t__bindgen_ty_1 {
    pub __bindgen_anon_1: vlib_stats_entry_t__bindgen_ty_1__bindgen_ty_1,
    pub index: u64,
    pub value: u64,
    pub data: *mut ::std::os::raw::c_void,
    pub string_vector: *mut *mut u8,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct vlib_stats_entry_t__bindgen_ty_1__bindgen_ty_1 {
    pub index1: u32,
    pub index2: u32,
}

#[repr(C)]
struct VlibStatsEntry {
    pub type_: stat_directory_type_t,
    pub __bindgen_anon_1: vlib_stats_entry_t__bindgen_ty_1,
    pub name: [::std::os::raw::c_char; VLIB_STATS_MAX_NAME_SZ],
}

#[repr(C)]
struct VlibStatsSharedHeader {
    version: u64,
    base: *const u8,
    epoch: u64,                              /* volatile */
    in_progress: u64,                        /* volatile */
    directory_vector: *const VlibStatsEntry, /* volatile */
}

#[repr(C)]
struct StatClientMain {
    current_epoch: u64,
    shared_header: *const VlibStatsSharedHeader,
    directory_vector: *const VlibStatsEntry,
    memory_size: u64,
    timeout: u64,
}

const TestChecker1: [u8; std::mem::size_of::<StatClientMain>()] =
    [0; std::mem::size_of::<sys::stat_client_main_t>()];
const TestChecker2: [u8; std::mem::size_of::<VlibStatsSharedHeader>()] =
    [0; std::mem::size_of::<sys::vlib_stats_shared_header_t>()];
const TestChecker3: [u8; std::mem::size_of::<VlibStatsEntry>()] =
    [0; std::mem::size_of::<sys::vlib_stats_entry_t>()];

pub struct VppStatClient {
    stat_client_ptr: *mut sys::stat_client_main_t,
    main: Option<StatClientMain>,
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
    vvec_ptr: *mut *mut u8,
}

impl VppStringVec {
    pub fn new() -> Self {
        let vvec_ptr = std::ptr::null_mut();
        VppStringVec { vvec_ptr }
    }

    pub fn push(&mut self, s: &str) {
        let cs = format!("{}\0", s);
        let cstr_ptr = cs.as_str() as *const str as *const [i8] as *const c_char;
        self.vvec_ptr = unsafe { sys::stat_segment_string_vector(self.vvec_ptr, cstr_ptr) };
    }

    pub fn len(&self) -> usize {
        let vv_len =
            unsafe { sys::stat_segment_vec_len(self.vvec_ptr as *mut libc::c_void) as usize };
        vv_len
    }
}

impl Index<usize> for VppStringVec {
    type Output = str;

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
}

impl fmt::Debug for VppStringVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.len() {
            write!(f, "{:?} ", &self[i]);
        }
        Ok(())
    }
}

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
    dir_ptr: *const u32,
    dir: &'a [u32],
}

impl Drop for VppStatDir<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::stat_segment_vec_free(self.dir_ptr as *mut libc::c_void);
        };
    }
}

pub struct VppStatData<'a> {
    stat_client_ptr: *mut sys::stat_client_main_t,
    data_ptr: *const sys::stat_segment_data_t,
    data: &'a [sys::stat_segment_data_t],
}

pub struct VppStatDataIterator<'a> {
    stat_data: &'a VppStatData<'a>,
    curr: usize,
}

impl<'a> Iterator for VppStatDataIterator<'a> {
    type Item = StatSegmentData<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.stat_data.len() {
            let curr = self.curr;
            self.curr = curr + 1;
            let cptr = self.stat_data.stat_client_ptr;
            Some(StatSegmentData::from_ctype(
                cptr,
                &self.stat_data.data[curr],
            ))
        } else {
            None
        }
    }
}

impl<'a> VppStatData<'a> {
    pub fn iter(&'a self) -> VppStatDataIterator {
        VppStatDataIterator {
            stat_data: self,
            curr: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.len() == 0
    }
}

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
}

impl<'a, 'b: 'a> VppStatDir<'a> {
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

    pub fn connect(path: &str) -> Result<Self, VppStatError> {
        use crate::VppStatError::*;
        use std::fs::File;
        use std::os::fd::FromRawFd;
        use std::os::fd::RawFd;
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
                return Err(CouldNotOpenSocket);
            }
            Err(e) => {
                println!("Couldn't connect {path}: {e:?}");
                return Err(CouldNotConnect);
            }
        };
    }
    pub fn connect_old(path: &str) -> Result<Self, VppStatError> {
        use crate::VppStatError::*;
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
                main: None,
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

    pub fn heartbeat(&self) -> f64 {
        unsafe { sys::stat_segment_heartbeat_r(self.stat_client_ptr) }
    }

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
