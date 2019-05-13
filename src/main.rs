pub mod cpu;
pub mod memory;
pub mod tia;


fn main() {
    use cpu::CPU;
    use memory::RAM;
    use tia::TIA;

    println!("Welcome player ONE!");
    let mut memory = RAM::new(&[]);
    let cpu = CPU::new(&mut memory);
    println!("{:#?}", cpu);

    let tia = TIA::new();
}
