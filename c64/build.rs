use common::build_utils::all_files_with_extension;
use common::build_utils::build_all_test_roms;
use common::build_utils::prepare_out_dir;
use std::error::Error;
use std::fs;
use std::path::Path;

fn copy_all_c64_roms() -> Result<(), Box<dyn Error>> {
    let rom_dest_path = prepare_out_dir(Path::new("roms"))?;

    let rom_source_path = Path::new("src").join("roms");
    let all_rom_files = all_files_with_extension(&rom_source_path, "bin")?;

    for rom_file_path in &all_rom_files {
        println!("Copying file '{}'.", rom_file_path.display());
        fs::copy(
            &rom_file_path,
            rom_dest_path.join(rom_file_path.file_name().unwrap()),
        )?;
    }

    for rom_file_path in &all_rom_files {
        println!("cargo:rerun-if-changed={}", rom_file_path.display());
    }
    println!("cargo:rerun-if-changed={}", rom_dest_path.display());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    copy_all_c64_roms()?;
    build_all_test_roms(&["--target", "c64"], &["--target", "c64"])?;
    Ok(())
}
