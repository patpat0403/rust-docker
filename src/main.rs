use std::process::{exit};
use std::env;
use std::fs::File;
use std::io::Write;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{sethostname, chroot, chdir, setgroups, setuid, setgid,getuid, getgid};
use nix::sys::wait::waitpid;
use nix::mount::{mount, umount2, MsFlags, MntFlags};
use nix::unistd::{fork, execvp, ForkResult};
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
    // Step 1: Unshare User namespace before forking
    // This is a critical step for permissions
    // ------------------------------------------------------------------------------------------------
    if let Err(e) = unshare(CloneFlags::CLONE_NEWUSER) {
        eprintln!("Failed to unshare User namespace: {}", e);
        exit(1);
    }

    // Get the current user's UID and GID, which will be 0 when run as root
    let uid = getuid();
    let gid = getgid();
    
    eprintln!("uid: {} ", uid );
    eprintln!("gid: {}", gid);
    
    // Set UID and GID to the mapped values (0 in this case)
    if let Err(e) = setuid(uid) {
        eprintln!("Failed to setuid in parent: {}", e);
        exit(1);
    }
    
    if let Err(e) = setgid(gid) {
        eprintln!("Failed to setgid in parent: {}", e);
        exit(1);
    }

    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            // ------------------------------------------------------------------------------------------------
            // Parent Process Logic
            // ------------------------------------------------------------------------------------------------
            waitpid(child, None).unwrap();
            
            // Unmount /proc from the parent process after the child exits
            if let Err(e) = umount2("/proc", MntFlags::MNT_DETACH) {
                eprintln!("Failed to unmount /proc filesystem: {}", e);
            }
            
            exit(0);
        }
        Ok(ForkResult::Child) => {
            // ------------------------------------------------------------------------------------------------
            // Child Process Logic
            // ------------------------------------------------------------------------------------------------

            // UID/GID Mapping and Privilege Dropping
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
            
            if let Err(e) = setuid(nix::unistd::Uid::from_raw(0)) {
                eprintln!("Failed to setuid in child: {}", e);
                exit(1);
            }
            
            if let Err(e) = setgid(nix::unistd::Gid::from_raw(0)) {
                eprintln!("Failed to setgid in child: {}", e);
                exit(1);
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
                None::<&str>,
            ) {
                eprintln!("Failed to mount /proc filesystem {}", e);
                exit(1);
            }

            // Final Step: Execute the command
            let path = CString::new(command_to_run.as_str()).unwrap();
            let args_c_string: Vec<CString> = command_args
                .iter()
                .map(|arg| CString::new(arg.as_str()).unwrap())
                .collect();
            
            execvp(&path, &args_c_string)
                .expect("Failed to execute command");
        }
        Err(e) => {
            eprintln!("Failed to fork: {}", e);
            exit(1);
        }
    }
}
