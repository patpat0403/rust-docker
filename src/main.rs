use std::process::exit;
use std::env;
use std::ffi::CString;
use std::ffi::CStr;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{sethostname, chroot, chdir, fork, ForkResult, execvp};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::mount::{mount, MsFlags};


fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Ensure the program is called with at least two arguments: "run" and the command to execute.
    if args.len() < 3 || args[1] != "run" {
        eprintln!("Usage: {} run <command> [args...]", args[0]);
        exit(1);
    }

    // We are now in the parent process. The goal is to fork a child.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            // Parent process waits for the child to finish.
            let status = waitpid(child, None).expect("Failed to wait for child");
            
            match status {
                WaitStatus::Exited(_, code) => exit(code),
                _ => {
                    eprintln!("Child process did not exit with a normal status");
                    exit(1);
                }
            }
        }
        Ok(ForkResult::Child) => {
            // This is the child process. We set up the container here.
            
            // 1. Create all necessary namespaces: UTS, PID, and Mount
            if let Err(e) = unshare(CloneFlags::CLONE_NEWUTS | CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNS) {
                eprintln!("Failed to unshare namespaces: {}", e);
                exit(1);
            }
            
            // 2. Set the hostname within the new UTS namespace
            if let Err(e) = sethostname("my-container-host") {
                eprintln!("Failed to set hostname: {}", e);
                exit(1);
            }
            
            // 3. Change the root filesystem
            if let Err(e) = chroot("alpine_fs") {
                eprintln!("Failed to set root filesystem: {}", e);
                exit(1);
            }
            
            if let Err(e) = chdir("/") {
                eprintln!("Failed to change to root dir in container: {}", e);
                exit(1);
            }
            
            // Mount the /proc filesystem
if let Err(e) = mount(
    Some(CString::new("proc").unwrap().as_ref()),
    CString::new("/proc").unwrap().as_ref(),
    Some(CString::new("proc").unwrap().as_ref()),
    MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
    None::<&CStr>,
) {
    eprintln!("Failed to mount /proc: {}", e);
    exit(1);
}

// 5. Execute the command
let command_args = &args[2..];
let command = CString::new(command_args[0].clone()).expect("Failed to create CString");
let c_args: Vec<CString> = command_args.iter().map(|arg| CString::new(arg.clone()).unwrap()).collect();

// Pass a slice of the CString vector directly to execvp
if let Err(e) = execvp(&command, &c_args) {
    eprintln!("Failed to execute command: {}", e);
    exit(1);
}
        }
        Err(e) => {
            eprintln!("Fork failed: {}", e);
            exit(1);
        }
    }
}