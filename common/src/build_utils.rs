use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::fs::DirEntry;
use std::io;
use std::path::Path;
use std::path::PathBuf;
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

/// Resolves a relative path against the output directory. Creates a directory
/// with this path if needed.
pub fn prepare_out_dir(relative_path: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join(relative_path);
    if !dest_path.exists() {
        fs::create_dir(&dest_path)?;
    }
    Ok(dest_path)
}

/// Returns paths to all files in a given directory that have given extension.
pub fn all_files_with_extension(dir_path: &Path, extension: &str) -> io::Result<Vec<PathBuf>> {
    let all_dir_entries: io::Result<Vec<DirEntry>> = fs::read_dir(&dir_path)?.collect();
    let all_dir_entries = all_dir_entries?;

    Ok(all_dir_entries
        .iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().map_or(false, |ext| ext == extension))
        .collect())
}

/// Builds all the ROM files from sources in the `src/test_roms` directory. Puts
/// the output in the `roms` subdirectory of the output directory. The `roms`
/// directory is created if it didn't exist. Returns an error if any file fails
/// to build.
///
/// In case of full success, this function also prints the
/// `cargo:rerun-if-changed` declarations on the standard output to tell Cargo to
/// rerun the build script only if relevant files change.
pub fn build_all_test_roms(
    assembler_args: &[&str],
    linker_args: &[&str],
) -> Result<(), Box<dyn Error>> {
    // Create the ROM destination directory if it doesn't exist.
    let dest_path = prepare_out_dir(Path::new("test_roms"))?;

    // Find all directory entries in the `src/test_roms` directory.
    let src_dir = Path::new("src").join("test_roms");
    let asm_files = all_files_with_extension(&src_dir, "s")?;
    let config_path = src_dir.join("build.cfg");

    // Assemble and link the files one by one. The `success` variable will
    // become `false` if any of these files fails to build.
    let mut success = true;
    for source_path in &asm_files {
        println!("Building file '{}'.", source_path.display());
        let result = build_rom(
            &source_path,
            &config_path,
            &dest_path,
            assembler_args,
            linker_args,
        );
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
    for path in asm_files {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    // Tell Cargo to also rerun the build script if the contents of the
    // source directory are changed (for example, a new file has been added).
    println!("cargo:rerun-if-changed={}", src_dir.display());

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
    assembler_args: &[&str],
    linker_args: &[&str],
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
        .arg(&output_path)
        .args(assembler_args);
    run_command(assembler_command)?;

    // Step 2: Link the output file.
    let bin_output_path = output_path.with_extension("bin");
    let mut linker_command = Command::new("cl65");
    linker_command
        .arg(&output_path)
        .arg("-C")
        .arg(&config_file)
        .arg("-o")
        .arg(&bin_output_path)
        .args(linker_args);
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
