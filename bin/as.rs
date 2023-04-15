use clap::Parser;
use rvsim::Program;
use std::{error::Error, path::PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path of the file to be assembled
    path: PathBuf,

    /// Path of the file to be written to.
    /// If not specified, the output will be written to stdout
    #[arg(short, long)]
    out: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let path = args.path;
    let program = Program::from_file(&path)?;

    if let Some(out) = args.out {
        program.write_file(&out.as_path())?;
    } else {
        program.print_stdout();
    }

    Ok(())
}
