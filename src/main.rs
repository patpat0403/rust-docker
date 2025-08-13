use std::process::{ exit };
use std::env;
use std::fs::File;
use std::io::Write;
use nix::sched::{ unshare, CloneFlags };
use nix::unistd::{ sethostname, chroot, chdir, setgroups, setuid, setgid, getuid, getgid };
use nix::sys::wait::waitpid;
use nix::mount::{ mount, umount2, MsFlags, MntFlags };
use nix::unistd::{ fork, execvp, ForkResult };
use std::ffi::CString;

fn run_command_in_container(command_to_run: &str, command_args: &[String]) {
    let uid = getuid();
    let gid = getgid();

    if let Err(e) = unshare(CloneFlags::CLONE_NEWUSER) {
        eprintln!("Failed to unshare User namespace: {}", e);
        exit(1);
    }

    if
        let Err(e) = unshare(
            CloneFlags::CLONE_NEWUTS | CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNS
        )
    {
        eprintln!("Failed to unshare namespaces: {}", e);
        exit(1);
    }

    if let Ok(mut uid_file) = File::create("/proc/self/uid_map") {
        if let Err(e) = uid_file.write_all(format!("0 {} 1", uid.as_raw()).as_bytes()) {
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
        if let Err(e) = gid_file.write_all(format!("0 {} 1", gid.as_raw()).as_bytes()) {
            eprintln!("Failed to write to gid_map: {}", e);
            exit(1);
        }
    }

    match (unsafe { fork() }) {
        Ok(ForkResult::Parent { child }) => {
            waitpid(child, None).unwrap();

            // Umount the /proc in the parent process, which is now in its own mount namespace
            if let Err(e) = umount2("/proc", MntFlags::MNT_DETACH) {
                eprintln!("Failed to unmount /proc filesystem: {}", e);
            }

            exit(0);
        }
        Ok(ForkResult::Child) => {
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

            // Mount /proc with correct flags
            if
                let Err(e) = mount(
                    Some("proc"),
                    "/proc",
                    Some("proc"),
                    MsFlags::empty(), // This is the key change
                    None::<&str>
                )
            {
                eprintln!("Failed to mount /proc filesystem {}", e);
                exit(1);
            }

            let path = CString::new(command_to_run).unwrap();
            let args_c_string: Vec<CString> = command_args
                .iter()
                .map(|arg| CString::new(arg.as_str()).unwrap())
                .collect();

            execvp(&path, &args_c_string).expect("Failed to execute command");
        }
        Err(e) => {
            eprintln!("Failed to fork: {}", e);
            exit(1);
        }
    }
}
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} run <command> [args...]", args[0]);
        exit(1);
    }

    let command_to_run = &args[2];
    let command_args = &args[3..];

    match args[1].as_str() {
        "run" => run_command_in_container(command_to_run, command_args),
        _ => eprintln!("Usage: {} run <command> [args...]", args[0]),
    }
}
