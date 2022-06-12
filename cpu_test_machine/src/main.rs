use clap::Parser;
use std::time::Duration;

use common::{
    app::CommonCliArguments,
    debugger::{adapter::TcpDebugAdapter, Debugger},
};
use ya6502::{
    cpu::{Cpu, MachineInspector},
    memory::Ram,
};

#[derive(Parser)]
struct Args {
    #[clap(flatten)]
    common: CommonCliArguments,
    test_file: String,
}

fn main() {
    let args = Args::parse();

    let test_program = std::fs::read(args.test_file).expect("Unable to read the test file");

    let mut ram = Box::new(Ram::new(16));
    ram.bytes[0x0000..=0xFFFF].copy_from_slice(&test_program);
    let mut cpu = Cpu::new(ram);
    cpu.jump_to(0x400);

    let mut debugger = if args.common.debugger {
        let mut dbg = Debugger::new(TcpDebugAdapter::new(args.common.debugger_port));
        if let Err(e) = dbg.update(&cpu) {
            eprintln!("Debugger error: {}", e);
        }
        Some(dbg)
    } else {
        None
    };

    let mut prev_pc = 0;

    loop {
        // println!("PC: ${:04X}", cpu.reg_pc());
        if let Some(debugger) = &mut debugger {
            debugger.process_messages(&cpu);
            if !debugger.stopped() {
                if let Err(e) = cpu.tick() {
                    eprintln!("CPU error: {}", e);
                    eprintln!("{}", &cpu);
                }
                if let Err(e) = debugger.update(&cpu) {
                    eprintln!("Debugger error: {}", e);
                }
            } else {
                // Yes, I know. Disgraceful. But it's so much easier than
                // supporting blocking mode in the debugger adapter.
                std::thread::sleep(Duration::from_millis(10));
            }
        } else {
            if let Err(e) = cpu.tick() {
                eprintln!("CPU error: {}", e);
                eprintln!("{}", &cpu);
            }
            if cpu.at_instruction_start() {
                let new_pc = cpu.reg_pc();
                if new_pc == prev_pc {
                    println!("{}", &cpu);
                    return;
                }
                prev_pc = new_pc;
            }
        }
    }
}
