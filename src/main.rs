pub mod address_space;
pub mod atari;
pub mod colors;
pub mod cpu;
pub mod frame_renderer;
pub mod memory;
pub mod tia;

pub mod test_utils;

fn main() {
    use address_space::AddressSpace;
    use memory::RAM;
    use tia::TIA;
    use atari::Atari;

    println!("Welcome player ONE!");
    let mut address_space = AddressSpace {
        tia: TIA::new(),
        ram: RAM::new(),
        rom: RAM::new(),
    };
    let atari = Atari::new(&mut address_space);
}
