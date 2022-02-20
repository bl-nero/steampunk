#![feature(assert_matches)]

pub mod app;
pub mod build_utils;
pub mod colors;
pub mod debugger;
pub mod test_utils;

#[cfg(test)]
#[macro_use]
#[no_link]
extern crate rustasm6502;
