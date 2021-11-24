#![feature(test)]

mod address_space;
mod c64;
mod frame_renderer;
mod vic;

use crate::c64::C64;
use vic::Vic;

fn main() {
    let mut c64 = C64::new().unwrap();
    c64.tick().unwrap();
}
