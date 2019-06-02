pub mod atari;
pub mod colors;
pub mod cpu;
pub mod frame_renderer;
pub mod memory;
pub mod tia;
pub mod address_space;

pub mod test_utils;

fn main() {
    use cpu::CPU;
    use memory::RAM;
    use tia::TIA;
    use address_space::AddressSpace;

    println!("Welcome player ONE!");
    let mut address_space = AddressSpace{
        tia: TIA::new(),
        ram: RAM::new(),
        rom: RAM::new(),
    };
    let cpu = CPU::new(&mut address_space);
    println!("{:#?}", cpu);
}
