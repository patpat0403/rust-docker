use std::process::{Command, exit};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Ensure the program is called with at least two arguments: "run" and the command to execute.
    if args.len() < 3 || args[1] != "run" {
        eprintln!("Usage: {} run <command> [args...]", args[0]);
        exit(1);
    }
    
    let command_to_run = &args[2];
    let command_args = &args[3..];

    // Create a new `Command` instance for the command we want to run.
    let mut child = Command::new(command_to_run)
        .args(command_args)
        .spawn()
        .expect("Failed to spawn command");

    // By default, `Command` will hook up the child process's stdin, stdout, and stderr
    // to the parent's streams. So, no additional code is needed here.
    
    // Wait for the child process to finish and get its exit status.
    let status = child.wait().expect("Failed to wait for child process");
    
    // Propagate the child's exit code.
    exit(status.code().unwrap_or(1));
}
