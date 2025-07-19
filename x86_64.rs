use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
#[rustfmt::skip]
pub enum Register {
    RAX, EAX, AX, AL,
    RBX, EBX, BX, BL,
    RCX, ECX, CX, CL,
    RDX, EDX, DX, DL,
    RSI, ESI, SI, SIL,
    RDI, EDI, DI, DIL,
    RBP, EBP, BP, BPL,
    RSP, ESP, SP, SPL,
    R8, R8D, R8W, R8B,
    R9, R9D, R9W, R9B,
    R10, R10D, R10W, R10B,
    R11, R11D, R11W, R11B,
    R12, R12D, R12W, R12B,
    R13, R13D, R13W, R13B,
    R14, R14D, R14W, R14B,
    R15, R15D, R15W, R15B,
}

impl Display for Register {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Size {
    BYTE,
    WORD,
    DWORD,
    QWORD,
}
impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{:?}", self).to_lowercase())
    }
}

struct Context {}
impl Context {
    pub fn variable_at(&self, v: Variable) -> impl Display {
        "foo".to_string()
    }
}

pub trait Compilable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, c: &Context) -> std::fmt::Result;
}

impl Compilable for dyn Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, c: &Context) -> std::fmt::Result {
        self.fmt(f)
    }
}

type Immediate = isize;
type Variable = usize;

#[derive(Debug, Clone, Copy)]
pub enum BaseAddr {
    Register(Register),
    Variable(Variable),
}

impl Compilable for BaseAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, c: &Context) -> std::fmt::Result {
        match self {
            Self::Register(r) => r.fmt(f),
            Self::Variable(v) => c.variable_at(*v).fmt(f),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Memory {
    size: Size,
    base_addr: BaseAddr,
    index_reg: Option<Register>,
    scale_value: Immediate,
    displacement: Immediate,
}
impl Compilable for Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, c: &Context) -> std::fmt::Result {
        write!(f, "{} [", self.size);
        self.base_addr.fmt(f, c)?;
        if let Some(r) = self.index_reg {
            write!(f, " + {}", r)?;
        }
        if self.scale_value > 0 {
            write!(f, "*{}", self.scale_value)?;
        }
        if self.displacement > 0 {
            write!(f, " + {}", self.displacement)?;
        }
        write!(f, "]")
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Address {
    Imm(Immediate),
    Reg(Register),
    Mem(Memory),
}

impl Compilable for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, c: &Context) -> std::fmt::Result {
        match self {
            Address::Imm(i) => i.fmt(f),
            Address::Reg(r) => r.fmt(f),
            Address::Mem(m) => m.fmt(f, c),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MOV {
    dest: Address,
    src: Address,
}

impl MOV {
    pub fn new(dest: Address, src: Address) -> MOV {
        match (&dest, &src) {
            (Address::Mem(_), Address::Mem(_)) => {
                panic!("Attempted to create MOV with both dest and src as addresses")
            }
            (Address::Imm(_), _) => {
                panic!("Attempted to create MOV with dest that's immediate")
            }
            _ => {}
        }
        MOV { dest, src }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    PUSH(Address),
    POP(Address),
    MOV(MOV),
    ADD(Address, Address),
}

pub mod Builder {
    use super::*;
    pub fn push(a: Address) -> Instruction {
        Instruction::PUSH(a)
    }
    pub fn mem(
        size: Size,
        base_addr: BaseAddr,
        index_reg: Option<Register>,
        scale_value: Immediate,
        displacement: Immediate,
    ) -> Memory {
        Memory {
            size,
            base_addr,
            index_reg,
            scale_value,
            displacement,
        }
    }
    pub const RAX: Address = Address::Reg(Register::RAX);
    pub const RBX: Address = Address::Reg(Register::RBX);
}

impl Compilable for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, c: &Context) -> std::fmt::Result {
        use Instruction::*;
        match self {
            POP(addr) => {
                f.write_str("pop ")?;
                addr.fmt(f, c)
            }
            PUSH(_) => todo!(),
            MOV(mov) => {
                f.write_str("mov ")?;
                mov.dest.fmt(f, c)?;
                f.write_str(", ")?;
                mov.src.fmt(f, c)
            }
            ADD(a, b) => {
                f.write_str("add ")?;
                a.fmt(f, c)?;
                f.write_str(", ")?;
                b.fmt(f, c)
            }
        }
    }
}

impl Compilable for Vec<Instruction> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, c: &Context) -> std::fmt::Result {
        for i in self {
            i.fmt(f, c)?;
            writeln!(f, "")?;
        }
        Ok(())
    }
}

mod test {
    use super::*;
    pub fn test_foo() {
        use Address::*;
        use Builder::*;
        use Instruction::*;
        use Size::*;
        let i = [POP(RAX), POP(RBX), ADD(RAX, RBX), push(RAX)];
        let RAM = BaseAddr::Variable(0);
        let j = [POP(Mem(mem(QWORD, RAM, None, 0, 8)))];
    }
}
