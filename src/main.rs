use std::process::{exit};
use std::env;
use std::fs::File;
use std::io::Write;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{sethostname, chroot, chdir, setgroups, setuid, setgid, fork, execvp};
use nix::sys::wait::waitpid;
use nix::mount::{mount, umount2, MsFlags, MntFlags};
use nix::unistd::ForkResult;
use std::ffi::CString;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 || args[1] != "run" {
        eprintln!("Usage: {} run <command> [args...]", args[0]);
        exit(1);
    }
    
    // ------------------------------------------------------------------------------------------------
    // Correct Order: Unshare User namespace before forking
    // ------------------------------------------------------------------------------------------------
    if let Err(e) = unshare(CloneFlags::CLONE_NEWUSER){
        eprintln!("Failed to unshare User namespace: {}", e);
        exit(1);
    }

    let command_to_run = &args[2];
    let command_args = &args[3..];

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            waitpid(child, None).unwrap();
            
            // Unmount /proc from the parent process after the child exits
            if let Err(e) = umount2("/proc", MntFlags::MNT_DETACH) {
                eprintln!("Failed to unmount /proc filesystem: {}", e);
            }
            
            exit(0);
        }
        Ok(ForkResult::Child) => {
            // Child process continues with the container setup
            
            // ------------------------------------------------------------------------------------------------
            // Step 2-4: Unshare other namespaces inside the child process
            // ------------------------------------------------------------------------------------------------
            if let Err(e) = unshare(
                CloneFlags::CLONE_NEWUTS |
                CloneFlags::CLONE_NEWPID |
                CloneFlags::CLONE_NEWNS
            ) {
                eprintln!("Failed to unshare namespaces: {}", e);
                exit(1);
            }

            // ------------------------------------------------------------------------------------------------
            // Step 5: UID/GID Mapping and Privilege Dropping
            // ------------------------------------------------------------------------------------------------
            if let Ok(mut uid_file) = File::create("/proc/self/uid_map") {
                if let Err(e) = uid_file.write_all(b"0 1000 1") {
                    eprintln!("Failed to write to uid_map: {}", e);
                    exit(1);
                }
            }
            // ... rest of your UID/GID mapping and privilege dropping logic here ...

            // ------------------------------------------------------------------------------------------------
            // Final Step: Execute the command
            // ------------------------------------------------------------------------------------------------
            let path = CString::new(command_to_run.as_str()).unwrap();
            let args_c_string: Vec<CString> = command_args
                .iter()
                .map(|arg| CString::new(arg.as_str()).unwrap())
                .collect();
            
            execvp(&path, &args_c_string)
                .expect("Failed to execute command");

            // No umount2 call needed here, as the parent handles it.
        }
        Err(e) => {
            eprintln!("Failed to fork: {}", e);
            exit(1);
        }
    }
}
