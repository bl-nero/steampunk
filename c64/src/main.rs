#![feature(test)]

mod address_space;
mod frame_renderer;
mod vic;

use std::cell::RefCell;
use std::rc::Rc;
use vic::Vic;
use ya6502::memory::Ram;

fn main() {
    let mut vic = Vic::new(Box::new(Ram::new(16)), Rc::new(RefCell::new(Ram::new(16))));
    vic.tick().unwrap();
    println!("Hello, world!");
}
