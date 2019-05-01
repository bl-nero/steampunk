mod cpu;
mod memory;

fn main() {
    use cpu::CPU;
    use memory::RAM;

    println!("Welcome player ONE!");
    let mut memory = RAM { bytes: &mut [] };
    let cpu = CPU::new(&mut memory);
}
