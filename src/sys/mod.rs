pub mod ptrace;

use crate::error::Error;
use crate::result::Result;
use libc::{
    __errno_location, c_int, dup2 as libcdup2, execvp as libcexecvp, fork as libcfork, pid_t,
    pipe as libcpipe, strerror as libcstrerror, wait as libcwait, WEXITSTATUS, WIFCONTINUED,
    WIFEXITED, WIFSIGNALED, WIFSTOPPED, WSTOPSIG, WTERMSIG,
};
use std::ffi::CString;
use std::fs::File;
use std::os::unix::io::{FromRawFd, RawFd};

pub enum Fork {
    Parent(pid_t),
    Child,
}

pub fn fork() -> Result<Fork> {
    match unsafe { libcfork() } {
        errno if errno < 0 => Err(Error::Errno(-errno)),
        0 => Ok(Fork::Child),
        pid => Ok(Fork::Parent(pid)),
    }
}

pub fn errwrap<F, T>(f: F) -> Result<T>
where
    F: FnOnce() -> T,
{
    unsafe { *__errno_location() = 0 };
    let result = f();
    match unsafe { *__errno_location() } {
        0 => Ok(result),
        errno => Err(Error::Errno(errno)),
    }
}

pub fn strerror(errno: c_int) -> Result<String> {
    let str_ptr = errwrap(|| unsafe { libcstrerror(errno) })?;
    let cs = unsafe { CString::from_raw(str_ptr) };
    Ok(cs.into_string()?)
}

pub fn execvp(cmd: &Vec<String>) -> Result<()> {
    if cmd.is_empty() {
        return Err("command cannot be empty".into());
    }

    let mut cstr_array = Vec::with_capacity(cmd.len());
    for arg in cmd {
        cstr_array.push(CString::new(arg.clone())?);
    }
    let mut ptr_array = Vec::with_capacity(cmd.len());
    for arg in &cstr_array {
        ptr_array.push(arg.as_ptr());
    }

    errwrap(|| unsafe {
        libcexecvp(*ptr_array.first().unwrap(), ptr_array.as_ptr());
    })
}

#[derive(Debug)]
pub enum WaitStatus {
    Stopped(pid_t, i32),
    Continued(pid_t),
    Exited(pid_t, i32),
    Signaled(pid_t, i32),
    Unknwon(pid_t, i32),
}

pub fn wait() -> Result<WaitStatus> {
    let mut status = 0;
    let pid = errwrap(|| unsafe { libcwait(&mut status) })?;

    let ws = if unsafe { WIFSTOPPED(status) } {
        let stopsig = unsafe { WSTOPSIG(status) };
        WaitStatus::Stopped(pid, stopsig)
    } else if unsafe { WIFEXITED(status) } {
        let exitstatus = unsafe { WEXITSTATUS(status) };
        WaitStatus::Exited(pid, exitstatus)
    } else if unsafe { WIFCONTINUED(status) } {
        WaitStatus::Continued(pid)
    } else if unsafe { WIFSIGNALED(status) } {
        let termsig = unsafe { WTERMSIG(status) };
        WaitStatus::Signaled(pid, termsig)
    } else {
        WaitStatus::Unknwon(pid, status)
    };

    Ok(ws)
}

pub fn pipe() -> Result<(File, File)> {
    let mut fds = [0 as RawFd; 2];
    errwrap(|| unsafe { libcpipe(fds.as_mut_ptr()) })?;
    let read = unsafe { File::from_raw_fd(fds[0]) };
    let write = unsafe { File::from_raw_fd(fds[1]) };
    Ok((read, write))
}

pub fn dup2(from: RawFd, to: RawFd) -> Result<()> {
    errwrap(|| unsafe { libcdup2(from, to) })?;
    Ok(())
}
