use common::build_utils::build_all_test_roms;
use std::process;

fn main() {
    if let Err(e) = build_all_test_roms(&[], &[]) {
        println!("{}", e);
        process::exit(1);
    }
}
