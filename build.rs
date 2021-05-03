use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process;
use std::process::Command;

#[derive(Debug)]
enum RomBuildError {
    ExternalToolFailed(i32),
    ExternalToolTerminated,
    BuildFailed,
}
use RomBuildError::*;

impl Error for RomBuildError {}

impl fmt::Display for RomBuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ExternalToolFailed(status) => {
                write!(f, "External tool failed with status code {}", status)
            }
            ExternalToolTerminated => write!(f, "External tool terminated"),
            BuildFailed => write!(f, "Failed to build ROMs"),
        }
    }
}

fn main() {
    if let Err(e) = build_all_roms() {
        println!("{}", e);
        process::exit(1);
    }
}

/// Builds all the ROM files from sources in the `src/asm` directory. Puts the
/// output in the `roms` subdirectory of the output directory. The `roms`
/// directory is created if it didn't exist. Returns an error if any file fails
/// to build.
///
/// In case of full success, this function also prints the
/// `cargo:rerun-if-changed` declarations on the standard output to tell Cargo to
/// rerun the build script only if relevant files change.
fn build_all_roms() -> Result<(), Box<dyn Error>> {
    // Create the ROM destination directory if it doesn't exist.
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("roms");
    if !dest_path.exists() {
        fs::create_dir(&dest_path)?;
    }

    // Find all directory entries in the `src/asm` directory.
    let asm_path = Path::new("src").join("asm");
    let all_dir_entries: Result<Vec<_>, _> = fs::read_dir(&asm_path)?.collect();
    let all_dir_entries = all_dir_entries?;

    let config_path = asm_path.join("atari2600.cfg");

    // Filter the directory entries to find all ASM files.
    let asm_files = all_dir_entries.iter().filter(|entry| {
        let path = entry.path();
        path.is_file() && path.extension().map_or(false, |ext| ext == "s")
    });

    // Assemble and link the files one by one. The `success` variable will
    // become `false` if any of these files fails to build.
    let mut success = true;
    for source_file in asm_files {
        let source_path = source_file.path();
        println!("Building file '{}'.", source_path.display());
        let result = build_rom(&source_path, &config_path, &dest_path);
        if let Err(err) = &result {
            println!(
                "Error while building file '{}': {}",
                source_path.display(),
                err
            );
            success = false;
        }
    }

    if !success {
        return Err(BuildFailed.into());
    }

    // Tell Cargo to rerun the build script only if relevant files change. It's
    // important to do it AFTER the files are assembled. Otherwise, a failed
    // build could silently "pass" on the next run, simply because it wouldn't
    // be retried at all.
    for entry in all_dir_entries.iter() {
        println!("cargo:rerun-if-changed={}", entry.path().display());
    }

    // Tell Cargo to also rerun the build script if the contents of the
    // `src/asm` directory are changed (for example, a new file has been added).
    println!("cargo:rerun-if-changed={}", asm_path.display());

    // Finally, a success!
    Ok(())
}

/// Assembles and links a single `source_file`. The output is stored in the
/// `dest_path` directory. Uses the specified `config_file` for linking the
/// binary.
fn build_rom(
    source_file: &Path,
    config_file: &Path,
    dest_path: &Path,
) -> Result<(), Box<dyn Error>> {
    // Compute the ROM file path out of the destination path and the original
    // source file name.
    let source_file_name = source_file.file_name();
    let output_path = match source_file_name {
        Some(name) => dest_path.join(name).with_extension("o"),
        None => dest_path.join("a.out"),
    };

    // Step 1: Assemble the file.
    let mut assembler_command = Command::new("ca65");
    assembler_command
        .arg(&source_file)
        .arg("-o")
        .arg(&output_path);
    run_command(assembler_command)?;

    // Step 2: Link the output file.
    let bin_output_path = output_path.with_extension("bin");
    let mut linker_command = Command::new("cl65");
    linker_command
        .arg(&output_path)
        .arg("-C")
        .arg(&config_file)
        .arg("-o")
        .arg(&bin_output_path);
    run_command(linker_command)
}

/// Runs a `command` and returns an error if it's not been successful.
fn run_command(command: Command) -> Result<(), Box<dyn Error>> {
    let mut command = command;
    println!("Running command: {:?}", &command);
    let status = command.status()?;

    return if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => Err(ExternalToolFailed(code).into()),
            None => Err(ExternalToolTerminated.into()),
        }
    };
}
