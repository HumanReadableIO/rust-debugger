mod error;
mod result;
mod safe;

use crate::result::Result;
use crate::safe::{fork, Fork};
use libc::pid_t;

fn main() -> Result<()> {
    match fork()? {
        Fork::Parent(child_pid) => parent(child_pid),
        Fork::Child => child(),
    };

    Ok(())
}

fn parent(child_pid: pid_t) {
  println!("Hello from parent! Child is {}.", child_pid);
}

fn child() {
  println!("Hello from child!");
}