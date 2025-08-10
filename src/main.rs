use std::process::{Command, exit};
use std::env;
use std::fs::File;
use std::io::Write;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{sethostname, chroot, chdir, setgroups, setuid, setgid};
use nix::mount::{mount, umount2, MsFlags, MntFlags};

// A helper function to run the command inside the container
fn run_container(args: &[String]) -> Result<(), NulError> {
    // Command and arguments for the execvp call
    let command = CString::new(args[0].clone())?;
    let c_args: Vec<CString> = args.iter().map(|arg| CString::new(arg.clone()).unwrap()).collect();
    let c_args_ptr: Vec<_> = c_args.iter().map(|s| s.as_ptr()).collect();
    
    // Perform the execvp call. This replaces the current process with the new command.
    execvp(&command, &c_args_ptr)?;

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 || args[1] != "run" {
        eprintln!("Usage: {} run <command> [args...]", args[0]);
        exit(1);
    }
    
    // We are now in the parent process. The goal is to fork a child.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            // Parent process waits for the child to finish.
            let status = waitpid(child, None).expect("Failed to wait for child");
            exit(status.code().unwrap_or(1));
        }
        Ok(ForkResult::Child) => {
            // This is the child process. We set up the container here.
            
            // 1. Create all necessary namespaces
            if let Err(e) = unshare(CloneFlags::CLONE_NEWUTS | CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNS) {
                eprintln!("Failed to unshare namespaces: {}", e);
                exit(1);
            }
            
            // 2. Set the hostname
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
            
            // 4. Mount the /proc filesystem
            if let Err(e) = mount(
                Some("proc"),
                "/proc",
                Some("proc"),
                MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
                None,
            ) {
                eprintln!("Failed to mount /proc: {}", e);
                exit(1);
            }

            // 5. Execute the command
            let command_args = &args[2..];
            if let Err(e) = run_container(command_args) {
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
// fn main() {
//     let args: Vec<String> = env::args().collect();
    
//     // Ensure the program is called with at least two arguments: "run" and the command to execute.
//     if args.len() < 3 || args[1] != "run" {
//         eprintln!("Usage: {} run <command> [args...]", args[0]);
//         exit(1);
//     }
          

//     if let Err(e) = unshare(CloneFlags::CLONE_NEWUTS){
//         eprintln!("Failed to unshare UTS namespace {}", e);
//         exit(1);
//     }

//     if let Err(e) = unshare(CloneFlags::CLONE_NEWPID){
//         eprintln!("Failed to create a new process {}", e);
//         exit(1);
//     }

//     // Create a new Mount namespace for filesystem isolation
//     if let Err(e) = unshare(CloneFlags::CLONE_NEWNS) {
//         eprintln!("Failed to unshare Mount namespace {}", e);
//         exit(1);
//     }

//     if let Err(e) = sethostname("my-container-host"){
//         eprintln!("Failed to set hostname {}", e);
//         exit(1);
//     }
    

//     //step 3 change root filesystem

//     if let Err(e) = chroot("alpine_fs"){
//         eprintln!("Failed to set root filesystem {}", e);
//         exit(1);
//     }

//     if let Err(e) = chdir("/"){
//         eprintln!("Failed to change to root dir in container {}", e);
//         exit(1);
//     }


//     let command_to_run = &args[2];
//     let command_args = &args[3..];

//     // Create a new `Command` instance for the command we want to run.
//     let mut child = Command::new(command_to_run)
//         .args(command_args)
//         .spawn()
//         .expect("Failed to spawn command");

//     // By default, `Command` will hook up the child process's stdin, stdout, and stderr
//     // to the parent's streams. So, no additional code is needed here.
    
//     // Wait for the child process to finish and get its exit status.
//     let status = child.wait().expect("Failed to wait for child process");

   
//     // Propagate the child's exit code.
//     exit(status.code().unwrap_or(1));
// }
