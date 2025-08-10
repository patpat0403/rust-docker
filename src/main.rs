use std::process::{exit};
use std::env;
use std::ffi::{CString, CStr};
use std::fs::File;
use std::io::Write;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{sethostname, chroot, chdir, setgroups, setuid, setgid, getuid, getgid};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::mount::{mount, MsFlags, umount2, MntFlags};
use nix::unistd::{fork, execvp, ForkResult};

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 || args[1] != "run" {
        eprintln!("Usage: {} run <command> [args...]", args[0]);
        exit(1);
    }
    
    // Get the current user's UID and GID
    let uid = getuid();
    let gid = getgid();

    // The most critical step: unshare the user namespace before forking
    if let Err(e) = unshare(CloneFlags::CLONE_NEWUSER) {
        eprintln!("Failed to unshare User namespace: {}", e);
        exit(1);
    }
    
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child, .. }) => {
            let status = waitpid(child, None).expect("Failed to wait for child");
            
            if let Err(e) = umount2("/proc", MntFlags::MNT_DETACH) {
                eprintln!("Failed to unmount /proc filesystem: {}", e);
            }
            
            match status {
                WaitStatus::Exited(_, code) => exit(code),
                _ => {
                    eprintln!("Child process did not exit with a normal status");
                    exit(1);
                }
            }
        }
        Ok(ForkResult::Child) => {
            // ------------------------------------------------------------------------------------------------
            // Child Process Logic
            // ------------------------------------------------------------------------------------------------

            // UID/GID Mapping (must be done after unshare(CLONE_NEWUSER))
            let uid_map = format!("0 {} 1", uid.as_raw());
            let gid_map = format!("0 {} 1", gid.as_raw());

            if let Ok(mut uid_file) = File::create("/proc/self/uid_map") {
                if let Err(e) = uid_file.write_all(uid_map.as_bytes()) {
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
                if let Err(e) = gid_file.write_all(gid_map.as_bytes()) {
                    eprintln!("Failed to write to gid_map: {}", e);
                    exit(1);
                }
            }
            
            // Unshare other namespaces inside the child process
            if let Err(e) = unshare(
                CloneFlags::CLONE_NEWUTS |
                CloneFlags::CLONE_NEWPID |
                CloneFlags::CLONE_NEWNS
            ) {
                eprintln!("Failed to unshare namespaces: {}", e);
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
                None::<&CStr>,
            ) {
                eprintln!("Failed to mount /proc filesystem {}", e);
                exit(1);
            }
            
            // Execute the command
            let command_args = &args[2..];
            let command = CString::new(command_args[0].clone()).expect("Failed to create CString");
            let c_args: Vec<CString> = command_args.iter().map(|arg| CString::new(arg.clone()).unwrap()).collect();
            
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