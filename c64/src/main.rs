#![feature(test)]

mod vic;

use std::cell::RefCell;
use std::rc::Rc;
use vic::Vic;
use ya6502::memory::SimpleRam;

fn main() {
    let mut vic = Vic::new(
        Box::new(SimpleRam::new()),
        Rc::new(RefCell::new(SimpleRam::new())),
    );
    vic.tick().unwrap();
    println!("Hello, world!");
}
