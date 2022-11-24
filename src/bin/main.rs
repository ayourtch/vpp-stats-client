use vpp_stat_client::*;

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

#[no_mangle]
fn check(sc: *mut stat_client_main_t, ptr: *mut u32, length: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let buf = vv2slice(ptr);
    for i in 0..length {
        let name = unsafe { stat_segment_index_to_name_r(buf[i], sc) };
        out.push(ptr2str(name).to_string());
    }
    out
}

fn do_dump(sc: *mut stat_client_main_t, ptr: *const stat_segment_data_t) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let buf = vv2slice(ptr);
    for i in 0..buf.len() {
        print!("Name: {} type: ", ptr2str(buf[i].name));
        match buf[i].type_ {
            STAT_DIR_TYPE_ILLEGAL => {
                unimplemented!()
            }
            STAT_DIR_TYPE_SCALAR_INDEX => {
                println!("SCALAR_INDEX : value {}", unsafe {
                    buf[i].__bindgen_anon_1.scalar_value
                });
            }
            STAT_DIR_TYPE_COUNTER_VECTOR_SIMPLE => {
                println!("COUNTER_VECTOR_SIMPLE");
                let vs = vv2slice(unsafe { buf[i].__bindgen_anon_1.simple_counter_vec });

                for k in 0..vs.len() {
                    let vss = vv2slice(vs[k]);
                    for j in 0..vss.len() {
                        println!("     [ {} @ {} ]: {} packets", j, k, vss[j]);
                    }
                }
            }
            STAT_DIR_TYPE_COUNTER_VECTOR_COMBINED => {
                println!("COUNTER_VECTOR_COMBINED");
                let vc = vv2slice(unsafe { buf[i].__bindgen_anon_1.combined_counter_vec });

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
                let nv = vv2slice(unsafe { buf[i].__bindgen_anon_1.name_vector });

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

fn main() {
    unsafe {
        let data = [0u8; 128];

        clib_mem_init(std::ptr::null_mut(), 64000000);

        let sc = stat_client_get();
        let rv = stat_segment_connect_r(cstr!("/tmp/stats.sock"), sc);
        println!("Hello world! {}", rv);
        println!("running dir");
        let patterns = std::ptr::null_mut();
        let dir = stat_segment_ls_r(patterns, sc);

        let str_buf = check(
            sc,
            dir,
            stat_segment_vec_len(dir as *mut libc::c_void) as usize,
        );

        // println!("{:?}", str_buf);
        /*
        for s in str_buf {
            println!("{}", s);
        }
        */

        println!("running dump");

        let res = stat_segment_dump_r(dir, sc);

        do_dump(sc, res);

        stat_segment_data_free(res);

        stat_segment_disconnect_r(sc);
        stat_client_free(sc);
    }
}
