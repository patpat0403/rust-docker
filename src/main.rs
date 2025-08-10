use std::process::{Command, exit};
use std::env;
use std::fs::File;
use std::io::Write;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{sethostname, chroot, chdir, setgroups, setuid, setgid};
use std::ffi::CString;
use std::ffi::NulError;
use nix::unistd::{fork, execvp};
use nix::sys::wait::waitpid;
use nix::unistd::ForkResult;
use nix::mount::{mount, umount2, MsFlags, MntFlags};


fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Ensure the program is called with at least two arguments: "run" and the command to execute.
    if args.len() < 3 || args[1] != "run" {
        eprintln!("Usage: {} run <command> [args...]", args[0]);
        exit(1);
    }
          

    if let Err(e) = unshare(CloneFlags::CLONE_NEWUTS){
        eprintln!("Failed to unshare UTS namespace {}", e);
        exit(1);
    }

    if let Err(e) = unshare(CloneFlags::CLONE_NEWPID){
        eprintln!("Failed to create a new process {}", e);
        exit(1);
    }

    // Create a new Mount namespace for filesystem isolation
    if let Err(e) = unshare(CloneFlags::CLONE_NEWNS) {
        eprintln!("Failed to unshare Mount namespace {}", e);
        exit(1);
    }

    if let Err(e) = sethostname("my-container-host"){
        eprintln!("Failed to set hostname {}", e);
        exit(1);
    }
    

    //step 3 change root filesystem

    if let Err(e) = chroot("alpine_fs"){
        eprintln!("Failed to set root filesystem {}", e);
        exit(1);
    }

    if let Err(e) = chdir("/"){
        eprintln!("Failed to change to root dir in container {}", e);
        exit(1);
    }

    if let Err(e) = mount(
    Some("proc"),
    "/proc",
    Some("proc"),
    MsFlags::empty(),
    None::<&str>,
) {
    eprintln!("Failed to mount /proc filesystem: {}", e);
    // This is the line that is failing. The specific error code is key.
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
