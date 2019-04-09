#[derive(Debug)] //this generates function that translates CPU to text
struct CPU<'a>{
    program_counter:u16, // u means unsigned and 16 means it is 16 bit
    accumulator:u8,
    memory: &'a mut RAM // & means reference
}
#[derive(Debug)]
struct RAM{
    bytes:[u8;25], //this means that computer has 25 u8's
}
fn main() {
    let mut memory = RAM{bytes: [0; 25]};
    let mut cpu = CPU{program_counter:0, accumulator:0, memory: &mut memory};
    println!("{:#?}", cpu ); //{:#?} formats cpu using #[derive(Debug)]
    println!("welcome player ONE!");
}
