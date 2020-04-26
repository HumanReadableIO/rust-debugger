use crate::debugger::{DebugInfo, Registers, Symbol};

use crate::result::Result;
use crate::sys::{Fork::*, WaitStatus::*, *};
use std::collections::HashMap;
use std::fs::File;

pub struct Subordinate {
    pid: i32,
    registers: Registers,
    stack: Vec<usize>,
    wait_status: WaitStatus,
    breakpoints: HashMap<usize, usize>,
    debug_info: DebugInfo,
}

impl Subordinate {
    pub fn spawn(cmd: Vec<String>) -> Result<Self> {
        info!("spawning with cmd: {:?}", cmd);

        let pid = match fork()? {
            Parent(child_pid) => child_pid,
            Child => {
                ptrace::traceme()?;
                execvp(&cmd)?;
                0
            }
        };

        let debug_info = DebugInfo::new(File::open(&cmd[0])?)?;

        let mut subordinate = Subordinate {
            pid,
            wait_status: WaitStatus::Unknwon(0, 0),
            registers: Registers::default(),
            stack: Vec::new(),
            breakpoints: HashMap::new(),
            debug_info,
        };

        subordinate.fetch_state()?;
        Ok(subordinate)
    }

    pub fn step(&mut self) -> Result<()> {
        ptrace::singlestep(self.pid)?;
        self.fetch_state()?;
        Ok(())
    }

    pub fn cont(&mut self) -> Result<()> {
        ptrace::cont(self.pid)?;
        self.fetch_state()?;
        Ok(())
    }

    pub fn peek(&self, addr: usize) -> Result<usize> {
        ptrace::peek(self.pid, addr)
    }

    pub fn poke(&self, addr: usize, data: usize) -> Result<()> {
        ptrace::poke(self.pid, addr, data)
    }

    pub fn read_bytes(&self, from: usize, size: usize) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(size);
        let wordlen = std::mem::size_of::<usize>();
        for i in 0..(size / wordlen) + 1 {
            for byte in self.peek(from + wordlen * i)?.to_ne_bytes().iter() {
                bytes.push(*byte);
                if bytes.len() == size {
                    break;
                }
            }
        }
        Ok(bytes)
    }

    pub fn read_words(&self, from: usize, size: usize) -> Result<Vec<usize>> {
        let mut words = Vec::with_capacity(size);
        let wordlen = std::mem::size_of::<usize>();
        for i in 0..size {
            words.push(self.peek(from + wordlen * i)?);
        }
        Ok(words)
    }

    pub fn exit_status(&self) -> Option<i32> {
        if let Exited(_, status) = self.wait_status {
            return Some(status);
        }
        None
    }

    pub fn breakpoint(&mut self, addr: usize) -> Result<()> {
        if let Some(_) = self.breakpoints.get(&addr) {
            return Ok(());
        }

        let data = self.peek(addr)?;
        self.poke(addr, data & (usize::max_value() - 255) | 0xcc)?;
        self.breakpoints.insert(addr, data);
        Ok(())
    }

    pub fn registers(&self) -> &Registers {
        &self.registers
    }

    pub fn instructions(&self, symbol: &Symbol) -> Result<Vec<u8>> {
        Ok(self.read_bytes(symbol.low_pc as usize, symbol.high_pc as usize)?)
    }

    pub fn stack(&self) -> &[usize] {
        &self.stack
    }

    pub fn debug_info(&self) -> &DebugInfo {
        &self.debug_info
    }

    fn fetch_state(&mut self) -> Result<()> {
        self.wait_status = wait()?;
        if let Stopped(_, _) = self.wait_status {
            self.registers = ptrace::getregs(self.pid)?.into();
            self.stack = self.read_words(self.registers.rsp as usize, 16)?;
            self.handle_breakpoint()?;
        };
        Ok(())
    }

    fn handle_breakpoint(&mut self) -> Result<()> {
        let addr = (self.registers.rip - 1) as usize;
        if let Some(data) = self.breakpoints.remove(&addr) {
            info!("hit breakpoint: {:x}", addr);
            self.registers.rip = addr as u64;
            self.poke(self.registers.rip as usize, data)?;
            ptrace::setregs(self.pid, &self.registers.clone().into())?;
        }

        Ok(())
    }
}
