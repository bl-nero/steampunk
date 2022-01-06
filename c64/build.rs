use common::build_utils::absolute_src_path;
use common::build_utils::all_files_with_extension;
use common::build_utils::assemble_all;
use common::build_utils::link;
use common::build_utils::process_source;
use common::build_utils::rerun_if_any_changed;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let c64_rom_dir = absolute_src_path("roms")?;
    let c64_roms = all_files_with_extension(&c64_rom_dir, "bin")?;

    let test_rom_dir = absolute_src_path("test_roms")?;
    let test_rom_sources = all_files_with_extension(&test_rom_dir, "s")?;
    let config_path = test_rom_dir.join("build.cfg");

    let all_sources = [
        vec![c64_rom_dir, test_rom_dir, config_path.clone()],
        c64_roms.clone(),
        test_rom_sources.clone(),
    ]
    .concat();
    rerun_if_any_changed(all_sources);

    copy_all_c64_roms(c64_roms)?;
    build_all_test_roms(test_rom_sources, config_path)?;
    Ok(())
}

fn copy_all_c64_roms<P, I>(c64_roms: I) -> Result<(), Box<dyn Error>>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = P>,
{
    for rom in c64_roms {
        copy_to_out_dir(rom)?;
    }
    Ok(())
}

fn copy_to_out_dir<P: AsRef<Path>>(source_file: P) -> Result<PathBuf, Box<dyn Error>> {
    process_source(
        source_file,
        |path| path.into(),
        |source_file, output_path| {
            println!("Copying file '{}'.", source_file.display());
            fs::copy(&source_file, &output_path)?;
            Ok(())
        },
    )
}

fn build_all_test_roms<AP, I, CP>(asm_files: I, config_path: CP) -> Result<(), Box<dyn Error>>
where
    AP: AsRef<Path>,
    I: IntoIterator<Item = AP>,
    CP: AsRef<Path>,
{
    let object_files = assemble_all(asm_files, &["--target", "c64"])?;

    let (programs, libs): (Vec<_>, Vec<_>) = object_files
        .iter()
        .partition(|obj| obj.file_name().map_or(false, |name| name != "common.o"));

    for program in programs {
        let all_obj_files = [vec![program], libs.clone()].concat();
        link(&all_obj_files, &config_path, &["--target", "c64"])?;
    }
    Ok(())
}
