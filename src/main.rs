use clap::Parser;
use lazy_static::lazy_static;
use rvsim::{CpuState, Program, RunState};
use std::{
    error::Error,
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input assembly file
    path: PathBuf,

    /// Print pipeline info for each cycle
    #[arg(short, long)]
    verbose: bool,

    /// Print analysis info
    #[arg(short, long)]
    analysis: bool,

    /// Step running
    #[arg(short, long)]
    step: bool,
}

lazy_static! {
    static ref ARGS: Args = Args::parse();
}

fn main() -> Result<(), Box<dyn Error>> {
    let program = Program::from_file(&ARGS.path)?;
    let mut app = AppState::new(&program);

    let quit = Arc::new(AtomicBool::new(false));
    ctrlc::set_handler(move || {
        if quit.load(Relaxed) == true {
            std::process::exit(0);
        }

        println!("Ctrl-C pressed. If you want to quit, press Ctrl-C again.");
        quit.store(true, Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    if ARGS.verbose && app.cpu.cycle() == 0 {
        println!("{}", app.cpu);
    }

    if ARGS.step {
        app.step()?;
    } else {
        app.run()?;
    }

    if ARGS.analysis {
        app.analysis();
    }

    Ok(())
}

struct AppState {
    cpu: CpuState,
}

impl AppState {
    fn new(program: &Program) -> Self {
        let mut cpu = CpuState::new();
        cpu.load(&program);

        AppState { cpu }
    }

    fn step(&mut self) -> Result<(), String> {
        let mut buf = String::with_capacity(100);

        while let Ok(_) = io::stdin().read_line(&mut buf) {
            let state = self.cpu.step()?;
            if ARGS.verbose {
                println!("{}", self.cpu);
            }

            match state {
                RunState::Running => {}
                RunState::Exit(code) => {
                    if code == 0 {
                        println!("Succesfully exit!");
                    } else {
                        println!("Exit with code {}!", code);
                    }
                    break;
                }
                RunState::Break => {
                    println!("Program break!");
                }
            }
        }

        Ok(())
    }

    fn run(&mut self) -> Result<(), String> {
        loop {
            let state = self.cpu.step()?;
            if ARGS.verbose {
                println!("{}", self.cpu);
            }

            match state {
                RunState::Running => {}
                RunState::Exit(code) => {
                    if code == 0 {
                        println!("Succesfully exit!");
                    } else {
                        println!("Exit with code {}!", code);
                    }
                    break;
                }
                RunState::Break => {
                    println!("Program break!");
                    println!("Press Enter to continue.");

                    let mut buf = String::new();
                    io::stdin().read_line(&mut buf).map_err(|e| e.to_string())?;
                }
            }
        }

        Ok(())
    }

    fn analysis(&self) {
        println!("========== Analysis ==========");
        println!("All Cycle: {}", self.cpu.cycle() - 1);
    }
}
