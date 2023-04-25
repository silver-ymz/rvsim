use clap::Parser;
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

    /// Turn on to print pipeline info for each cycle
    #[arg(short, long)]
    verbose: bool,

    /// Turn on to print analysis info
    #[arg(short, long)]
    analysis: bool,

    #[arg(short, long)]
    step: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let path = args.path;
    let verbose = args.verbose;
    let analysis = args.analysis;
    let step = args.step;

    let program = Program::from_file(&path)?;
    let mut app = AppState::new(&program);
    let mut buf = String::new();

    let quit = Arc::new(AtomicBool::new(false));
    ctrlc::set_handler(move || {
        if quit.load(Relaxed) == true {
            std::process::exit(0);
        }

        println!("Ctrl-C pressed. If you want to quit, press Ctrl-C again.");
        quit.store(true, Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    if verbose && app.cpu.cycle() == 0 {
        println!("{}", app.cpu);
    }

    if step {
        while let Ok(_) = io::stdin().read_line(&mut buf) {
            app.step(verbose)?;
        }
    } else {
        app.run(verbose)?;
    }

    if analysis {
        app.analysis();
    }

    Ok(())
}

struct AppState {
    cpu: CpuState,
}

impl AppState {
    fn new(program: &Program) -> Self {
        let mut cpu = CpuState::default();
        cpu.load(&program);

        AppState { cpu }
    }

    fn step(&mut self, verbose: bool) -> Result<(), String> {
        self.cpu.step()?;
        if verbose {
            println!("{}", self.cpu);
        }

        Ok(())
    }

    fn run(&mut self, verbose: bool) -> Result<(), String> {
        loop {
            let state = self.cpu.step()?;
            if verbose {
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
        println!("Data Hazard: {}", self.cpu.data_hazard());
        println!("Control Hazard: {}", self.cpu.control_hazard());
        println!(
            "Stall Cycle: {}",
            self.cpu.data_hazard() + self.cpu.control_hazard()
        );
    }
}
