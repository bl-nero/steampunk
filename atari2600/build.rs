use common::build_utils::absolute_src_path;
use common::build_utils::all_files_with_extension;
use common::build_utils::assemble_all;
use common::build_utils::link;
use common::build_utils::rerun_if_any_changed;
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let test_rom_dir = absolute_src_path("test_roms")?;
    let test_rom_sources = all_files_with_extension(&test_rom_dir, "s")?;
    let config_path = test_rom_dir.join("build.cfg");

    let all_sources = [
        vec![test_rom_dir, config_path.clone()],
        test_rom_sources.clone(),
    ]
    .concat();
    rerun_if_any_changed(all_sources);

    build_all_test_roms(test_rom_sources, config_path)?;
    Ok(())
}

fn build_all_test_roms<AP, I, CP>(asm_files: I, config_path: CP) -> Result<(), Box<dyn Error>>
where
    AP: AsRef<Path>,
    I: IntoIterator<Item = AP>,
    CP: AsRef<Path>,
{
    let object_files = assemble_all(asm_files, &[])?;

    for program in object_files {
        link(&[program], &config_path, &[])?;
    }
    Ok(())
}
