pub mod colors;
pub mod cpu;
pub mod frame_renderer;
pub mod memory;
pub mod tia;

pub mod test_utils;

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
