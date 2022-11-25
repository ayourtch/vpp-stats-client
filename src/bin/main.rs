use vpp_stat_client::sys::*;

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
use std::str;

fn ptr2str(cstrptr: *const i8) -> &'static str {
    let c_str: &CStr = unsafe { CStr::from_ptr(cstrptr) };
    let str_slice: &str = c_str.to_str().unwrap();
    str_slice
}

fn vv2slice<T>(vv: *const T) -> &'static [T] {
    unsafe {
        let vv_len = stat_segment_vec_len(vv as *mut libc::c_void) as usize;
        let slice: &[T] = core::slice::from_raw_parts(vv, vv_len);
        slice
    }
}

struct CounterComnbined {
    packets: u64,
    bytes: u64,
}

enum StatValue<'a> {
    Illegal,
    ScalarIndex(f64),
    CounterVectorSimple(&'a [*const u64]),
    CounterVectorCombined(&'a [*const vlib_counter_t]),
    NameVector(&'a [*const i8]),
    Empty,
    Symlink,
}

struct StatSegmentData<'a> {
    name: &'a str,
    val: StatValue<'a>,
}

fn do_dump(sc: *mut stat_client_main_t, item: &stat_segment_data_t) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    print!("Name: {} type: ", ptr2str(item.name));
    match item.type_ {
        STAT_DIR_TYPE_ILLEGAL => {
            unimplemented!()
        }
        STAT_DIR_TYPE_SCALAR_INDEX => {
            println!("SCALAR_INDEX : value {}", unsafe {
                item.__bindgen_anon_1.scalar_value
            });
        }
        STAT_DIR_TYPE_COUNTER_VECTOR_SIMPLE => {
            println!("COUNTER_VECTOR_SIMPLE");
            let vs = vv2slice(unsafe { item.__bindgen_anon_1.simple_counter_vec });

            for k in 0..vs.len() {
                let vss = vv2slice(vs[k]);
                for j in 0..vss.len() {
                    println!("     [ {} @ {} ]: {} packets", j, k, vss[j]);
                }
            }
        }
        STAT_DIR_TYPE_COUNTER_VECTOR_COMBINED => {
            println!("COUNTER_VECTOR_COMBINED");
            let vc = vv2slice(unsafe { item.__bindgen_anon_1.combined_counter_vec });

            for k in 0..vc.len() {
                let vcs = vv2slice(vc[k]);

                for j in 0..vcs.len() {
                    println!(
                        "     [ {} @ {} ]: {} packets, {} bytes",
                        j, k, vcs[j].packets, vcs[j].bytes
                    );
                }
            }
        }
        STAT_DIR_TYPE_NAME_VECTOR => {
            println!("NAME_VECTOR");
            let nv = vv2slice(unsafe { item.__bindgen_anon_1.name_vector });

            for k in 0..nv.len() {
                println!("[{}]: {}", k, ptr2str(nv[k] as *const i8));
            }
        }
        STAT_DIR_TYPE_EMPTY => {
            println!("EMPTY");
        }
        STAT_DIR_TYPE_SYMLINK => {
            println!("SYMLINK");
        }
        7_u32..=u32::MAX => unimplemented!(),
    }
    out
}
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

struct VppStatClient {
    stat_client_ptr: *mut vpp_stat_client::sys::stat_client_main_t,
}

#[derive(Debug, Clone, PartialEq)]
enum VppStatError {
    CouldNotOpenSocket,
    CouldNotConnect,
    ReceivingFdFailed,
    MmapFstatFailed,
    MmapMapFailed,
}

struct VppStatDir<'a> {
    client: &'a VppStatClient,
    dir_ptr: *const u32,
}

struct VppStatData<'a> {
    dir: &'a VppStatDir<'a>,
    data_ptr: *const vpp_stat_client::sys::stat_segment_data_t,
    data: &'a [vpp_stat_client::sys::stat_segment_data_t],
}

struct VppStatDataIterator<'a> {
    stat_data: &'a VppStatData<'a>,
    curr: usize,
}

impl<'a> Iterator for VppStatDataIterator<'a> {
    type Item = &'a vpp_stat_client::sys::stat_segment_data_t;
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr < self.stat_data.len() {
            let curr = self.curr;
            self.curr = curr + 1;
            Some(&self.stat_data.data[curr])
        } else {
            None
        }
    }
}

impl<'a> VppStatData<'a> {
    fn iter(&'a self) -> VppStatDataIterator {
        VppStatDataIterator {
            stat_data: self,
            curr: 0,
        }
    }
    fn len(&self) -> usize {
        self.data.len()
    }
    fn is_empty(&self) -> bool {
        self.data.len() == 0
    }
}

impl Drop for VppStatData<'_> {
    fn drop(&mut self) {
        unsafe {
            stat_segment_data_free(self.data_ptr as *mut vpp_stat_client::sys::stat_segment_data_t)
        };
    }
}

impl<'a> VppStatDir<'a> {
    fn dump(&'a self) -> VppStatData<'a> {
        let res =
            unsafe { stat_segment_dump_r(self.dir_ptr as *mut u32, self.client.stat_client_ptr) };
        let res_len = unsafe { stat_segment_vec_len(res as *mut libc::c_void) as usize };
        let slice: &'a [vpp_stat_client::sys::stat_segment_data_t] =
            unsafe { core::slice::from_raw_parts(res, res_len) };
        VppStatData {
            dir: self,
            data_ptr: res,
            data: slice,
        }
    }
}

impl VppStatClient {
    fn connect(path: &str) -> Result<Self, VppStatError> {
        use crate::VppStatError::*;
        use vpp_stat_client::sys::*;

        let sc = unsafe { stat_client_get() };
        let cpath = format!("{}\0", path);
        let cstrpath = cpath.as_str() as *const str as *const [i8] as *const i8;
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

    fn ls(&self) -> VppStatDir {
        let patterns = std::ptr::null_mut();
        let dir_ptr = unsafe { stat_segment_ls_r(patterns, self.stat_client_ptr) };
        VppStatDir {
            client: &self,
            dir_ptr,
        } // FIXME: errors
    }
}

impl Drop for VppStatClient {
    fn drop(&mut self) {
        unsafe {
            stat_segment_disconnect_r(self.stat_client_ptr);
            stat_client_free(self.stat_client_ptr);
        }
    }
}

fn main() {
    unsafe {
        let data = [0u8; 128];

        clib_mem_init(std::ptr::null_mut(), 64000000);

        println!("running dir");

        let c = VppStatClient::connect("/tmp/stats.sock").unwrap();
        let dir = c.ls();
        /*
        let buf = vv2slice(ptr);
        for i in 0..length {
            let name = unsafe { stat_segment_index_to_name_r(buf[i], sc) };
            out.push(ptr2str(name).to_string());
        }


            let str_buf = check(
                sc,
                dir,
                stat_segment_vec_len(dir as *mut libc::c_void) as usize,
            );
            */

        // println!("{:?}", str_buf);
        /*
        for s in str_buf {
            println!("{}", s);
        }
        */

        println!("running dump");
        let data = dir.dump();
        for item in data.iter() {
            do_dump(data.dir.client.stat_client_ptr, item);
        }
    }
}
