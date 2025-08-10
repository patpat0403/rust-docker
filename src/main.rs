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
    
    let command_to_run = &args[2];
    let command_args = &args[3..];

    // ------------------------------------------------------------------------------------------------
    // Step 1: Fork the process
    // ------------------------------------------------------------------------------------------------
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            // Parent process waits for the child to complete
            waitpid(child, None).unwrap();
            exit(0);
        }
        Ok(ForkResult::Child) => {
            // Child process continues with the container setup
            
            // ------------------------------------------------------------------------------------------------
            // Step 2-4: Unshare all namespaces
            // ------------------------------------------------------------------------------------------------
            if let Err(e) = unshare(
                CloneFlags::CLONE_NEWUSER |
                CloneFlags::CLONE_NEWUTS |
                CloneFlags::CLONE_NEWPID |
                CloneFlags::CLONE_NEWNS
            ) {
                eprintln!("Failed to unshare namespaces: {}", e);
                exit(1);
            }

            // ------------------------------------------------------------------------------------------------
            // Step 5: UID/GID Mapping (now that we are in a new user namespace)
            // ------------------------------------------------------------------------------------------------
            if let Ok(mut uid_file) = File::create("/proc/self/uid_map") {
                if let Err(e) = uid_file.write_all(b"0 1000 1") {
                    eprintln!("Failed to write to uid_map: {}", e);
                    exit(1);
                }
            }

            if let Ok(mut setgroups_file) = File::create("/proc/self/setgroups") {
                if let Err(e) = setgroups_file.write_all(b"deny") {
                    eprintln!("Failed to write to setgroups: {}", e);
                    exit(1);
                }
            }

            if let Ok(mut gid_file) = File::create("/proc/self/gid_map") {
                if let Err(e) = gid_file.write_all(b"0 1000 1") {
                    eprintln!("Failed to write to gid_map: {}", e);
                    exit(1);
                }
            }
            
            // Set UID and GID to 0 inside the new namespace
            if let Err(e) = setuid(nix::unistd::Uid::from_raw(0)) {
                eprintln!("Failed to setuid in child: {}", e);
                exit(1);
            }
            
            if let Err(e) = setgid(nix::unistd::Gid::from_raw(0)) {
                eprintln!("Failed to setgid in child: {}", e);
                exit(1);
            }
            
            // Set hostname, chroot, and mount /proc in the isolated environment
            if let Err(e) = sethostname("my-container-host") {
                eprintln!("Failed to set hostname {}", e);
                exit(1);
            }
            
            if let Err(e) = chroot("alpine_fs") {
                eprintln!("Failed to set root filesystem {}", e);
                exit(1);
            }
            
            if let Err(e) = chdir("/") {
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
                eprintln!("Failed to mount /proc filesystem {}", e);
                exit(1);
            }

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

            // Cleanup happens after the child process exits
            if let Err(e) = umount2("/proc", MntFlags::MNT_DETACH) {
                eprintln!("Failed to unmount /proc filesystem {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to fork: {}", e);
            exit(1);
        }
    }
}
