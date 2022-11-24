#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused_variables, unused_mut)]
#![allow(unused)]

#[macro_use]
pub mod macros; /* Handy macros */

// use std;
use std::fmt::{Debug, Error, Formatter};

pub mod sys {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

// unsafe impl std::marker::Send for  vlib_plugin_registration_t { }

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
