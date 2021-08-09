#![feature(test)]
#![recursion_limit = "256"] // For assembly macros with long content

#[cfg(test)]
#[macro_use]
#[no_link]
extern crate rustasm6502;

pub mod cpu;
pub mod memory;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
