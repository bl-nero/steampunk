pub mod cpu;
pub mod memory;


fn main() {
    use cpu::CPU;
    use memory::RAM;

    println!("Welcome player ONE!");
    let mut memory = RAM::new(&[]);
    let cpu = CPU::new(&mut memory);
    println!("{:#?}", cpu);
}
