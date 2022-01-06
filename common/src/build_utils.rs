use std::env;
use std::env::VarError;
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
    ExternalToolFailed(Command, i32),
    ExternalToolTerminated(Command),
}
use RomBuildError::*;

impl Error for RomBuildError {}

impl fmt::Display for RomBuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExternalToolFailed(command, status) => {
                write!(
                    f,
                    "External tool failed with status code {}: {:?}",
                    status, command
                )
            }
            ExternalToolTerminated(command) => write!(f, "External tool terminated: {:?}", command),
        }
    }
}

/// Tells Cargo to rerun the build script only if relevant files change. It's
/// important to _always_ call it with a complete list of source files, or never
/// call it at all. Otherwise, some soures could be ignored, for example, in
/// case of a failed build.
pub fn rerun_if_any_changed<I, P>(paths: I)
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.as_ref().display());
    }
}

/// Returns an absolute path of the `src` directory.
fn absolute_src_dir() -> io::Result<PathBuf> {
    env::current_dir().map(|d| d.join("src"))
}

/// Returns an absolute path of a path relative to the `src` directory.
pub fn absolute_src_path<P: AsRef<Path>>(relative_path: P) -> io::Result<PathBuf> {
    absolute_src_dir().map(|d| d.join(relative_path))
}

/// Returns an absolute path of a path relative to the crate's output directory.
fn absolute_out_path<P: AsRef<Path>>(relative_path: P) -> Result<PathBuf, VarError> {
    env::var("OUT_DIR").map(|d| PathBuf::from(d).join(relative_path))
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

/// Creates a parent directory of a given path if it doesn't exist.
fn ensure_parent_exists(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        println!("Creating directory {}", parent.display());
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Assembles all files and returns a vector of object file paths.
pub fn assemble_all<P, I>(
    asm_files: I,
    assembler_args: &[&str],
) -> Result<Vec<PathBuf>, Box<dyn Error>>
where
    P: AsRef<Path>,
    I: IntoIterator<Item = P>,
{
    asm_files
        .into_iter()
        .map(|source| assemble(source, assembler_args))
        .collect()
}

/// Assembles a single source file using `ca65` and places results in the
/// crate's output directory. Returns the output file's path.
fn assemble<P: AsRef<Path>>(
    source_file: P,
    assembler_args: &[&str],
) -> Result<PathBuf, Box<dyn Error>> {
    process_source(
        source_file,
        |path| path.with_extension("o"),
        |source_file, output_path| {
            println!("Assembling file '{}'.", &source_file.display());
            let mut assembler_command = Command::new("ca65");
            assembler_command
                .arg(&source_file)
                .arg("-o")
                .arg(&output_path)
                .args(assembler_args);
            run_command(assembler_command)
        },
    )
}

/// Processes a given source file using certain action and places the output in
/// the crate's output directory. The output file's name is taken from the
/// `to_out_name` function; if the output file's name should be the same as the
/// input one, the `to_out_name` function should simply return the argument
/// provided to it.
pub fn process_source<P, O, A>(
    source_file: P,
    to_out_name: O,
    action: A,
) -> Result<PathBuf, Box<dyn Error>>
where
    P: AsRef<Path>,
    O: FnOnce(&Path) -> PathBuf,
    A: FnOnce(&Path, &Path) -> Result<(), Box<dyn Error>>,
{
    let source_file = source_file.as_ref();
    let source_relative_path = source_file.strip_prefix(absolute_src_dir()?)?;
    let output_relative_path = to_out_name(source_relative_path);
    let output_absolute_path = absolute_out_path(&output_relative_path)?;

    ensure_parent_exists(&output_absolute_path)?;
    action(source_file, &output_absolute_path)?;

    Ok(output_absolute_path)
}

/// Links object files using `cl65` and places the output in the crate's output
/// directory.  The output file name is computed by taking the first object's
/// file name and changing its extension to `.bin`.
pub fn link<PO: AsRef<Path>, PC: AsRef<Path>>(
    object_files: &[PO],
    config_file: PC,
    linker_args: &[&str],
) -> Result<PathBuf, Box<dyn Error>> {
    let object_files: Vec<_> = object_files.iter().map(|f| f.as_ref()).collect();
    let config_file = config_file.as_ref();
    let bin_absolute_path = object_files[0].with_extension("bin");

    println!("Linking file '{}'.", &bin_absolute_path.display());
    let mut linker_command = Command::new("cl65");
    linker_command
        .args(object_files)
        .arg("-C")
        .arg(config_file)
        .arg("-o")
        .arg(&bin_absolute_path)
        .args(linker_args);
    run_command(linker_command)?;

    Ok(bin_absolute_path)
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
            Some(code) => Err(ExternalToolFailed(command, code).into()),
            None => Err(ExternalToolTerminated(command).into()),
        }
    };
}
