mod vic;

use vic::Vic;

fn main() {
    let mut vic = Vic::new();
    vic.tick();
    println!("Hello, world!");
}
