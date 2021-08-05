use anyhow::{anyhow, Context, Result};
use hackvm::{TokenizedProgram, VMSegment, VMToken};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::{cmp, fs};

use crate::Runtime;

fn compile_arithmetic(op: &VMToken) -> String {
    match op {
        VMToken::Add => "\
            pop     rax
            pop     rbx
            add     rax, rbx
            push    rax"
            .to_string(),
        VMToken::Sub => "\
            pop     rbx
            pop     rax
            sub     rax, rbx
            push    rax"
            .to_string(),
        VMToken::Neg => "\
            pop     rax
            mov     rbx, 0
            sub     rbx, rax
            push    rbx"
            .to_string(),
        VMToken::Not => "\
            pop     rax
            not     rax
            push    rax"
            .to_string(),
        VMToken::And => "\
            pop     rax
            pop     rbx
            and     rax, rbx
            push    rax"
            .to_string(),
        VMToken::Or => "\
            pop     rax
            pop     rbx
            or      rax, rbx
            push    rax"
            .to_string(),
        VMToken::Eq => "\
            pop     rax
            pop     rbx
            cmp     rax, rbx
            mov     rcx, -1
            cmove   rax, rcx
            mov     rcx, 0
            cmovne  rax, rcx
            push    rax"
            .to_string(),
        VMToken::Gt => "\
            pop     rbx
            pop     rax
            cmp     rax, rbx
            mov     rcx, -1
            cmovg   rax, rcx
            mov     rcx, 0
            cmovng  rax, rcx
            push    rax"
            .to_string(),
        VMToken::Lt => "\
            pop     rbx
            pop     rax
            cmp     rax, rbx
            mov     rcx, -1
            cmovl   rax, rcx
            mov     rcx, 0
            cmovnl  rax, rcx
            push    rax"
            .to_string(),
        _ => panic!("Token {:?} is not arithmetic", op),
    }
}

fn compile_pop(context: &CommandContext, segment: &VMSegment, index: &u16) -> String {
    match segment {
        VMSegment::Temp => {
            format!("pop     qword [TEMP + {}]", (index + 5) * 8)
        }
        VMSegment::Static => {
            format!(
                "pop     qword [{} + {}]",
                context.statics_var_name(),
                index * 8
            )
        }
        VMSegment::Argument => format!(
            "\
            mov rax, qword [rbp + 2*8]
            pop qword [rbp + 2*8 + rax*8 - {}*8]",
            index
        ),
        VMSegment::Local => format!("pop qword [rbp - {}]", (index + 1) * 8),
        VMSegment::Pointer => match index {
            0 => "pop r14".to_string(),
            1 => "pop r15".to_string(),
            _ => panic!("invalid index for pointer segment: {}", index),
        },
        VMSegment::This => format!("pop {}", context.get_address("r14", *index as usize)),
        VMSegment::That => format!("pop {}", context.get_address("r15", *index as usize)),
        VMSegment::Constant => panic!("Can't pop to constant segment"),
    }
}

fn compile_push(context: &CommandContext, segment: &VMSegment, index: &u16) -> String {
    match segment {
        VMSegment::Argument => format!(
            "\
            mov rax, qword [rbp + 2*8]
            push qword [rbp + 2*8 + rax*8 - {}*8]",
            index
        ),
        VMSegment::Local => format!("push qword [rbp - {}]", (index + 1) * 8),
        VMSegment::Pointer => match index {
            0 => "push r14".to_string(),
            1 => "push r15".to_string(),
            _ => panic!("invalid index for pointer segment: {}", index),
        },
        VMSegment::This => format!("push {}", context.get_address("r14", *index as usize)),
        VMSegment::That => format!("push {}", context.get_address("r15", *index as usize)),
        VMSegment::Constant => {
            let value = index;
            format!(
                "\
                        mov      rax, {}\n\
                        push     rax\n",
                value
            )
        }
        VMSegment::Static => {
            format!(
                "push     qword [{} + {}]",
                context.statics_var_name(),
                index * 8
            )
        }
        VMSegment::Temp => {
            format!(
                "\
                        push     qword [TEMP + {}]\n",
                (index + 5) * 8
            )
        }
    }
}

fn compile_function(func_name: &str, num_locals: &u16) -> String {
    let mut lines: String = String::new();
    lines.push_str(&format!("global {}\n{}:\n", func_name, func_name));
    lines.push_str(&format!("enter  {},0\n", num_locals * 8));
    for i in 0..*num_locals {
        lines.push_str(&format!("mov qword [rbp-{}], 0\n", (i + 1) * 8));
    }
    lines
}

fn compile_call_c(func_name: &str, num_args: &u16) -> String {
    let mut s = String::new();
    let mut registers = vec!["rdi", "rsi", "rdx", "rcx", "r8", "r9"];

    if func_name == "syscall" {
        registers = vec!["rax", "rdi", "rsi", "rdx", "r10", "r8", "r9"];
    }

    let num_args = cmp::min(*num_args as usize, registers.len());
    for i in 0..num_args {
        s.push_str("pop ");
        s.push_str(registers[num_args - 1 - i]);
        s.push_str("\n");
    }
    if func_name == "syscall" {
        s.push_str("syscall\n");
    } else {
        s.push_str(&format!("call {}\n", func_name));
    }
    s.push_str("push rax\n");
    return s;
}

fn compile_call(func_name: &str, num_args: &u16) -> String {
    if func_name.starts_with("c.") {
        return compile_call_c(func_name.split(".").nth(1).unwrap(), num_args);
    }
    let mut lines = String::new();
    // push this and that onto the stack
    lines.push_str("push r14\n");
    lines.push_str("push r15\n");

    // arguments will already be on the stack
    // push the number of arguments onto the stack
    lines.push_str(&format!("mov rdx, {}\n", num_args + 2));
    lines.push_str("push rdx\n");

    // actually call the function
    lines.push_str(&format!("call {}\n", func_name));

    // pop the number of arguments off the stack
    lines.push_str("pop rbx\n");

    // pop this and that from the stack
    lines.push_str("pop r15\n");
    lines.push_str("pop r14\n");

    // pop the arguments off the stack
    for _ in 0..(*num_args) {
        lines.push_str("pop rbx\n");
    }
    lines.push_str("push rax\n");
    lines
}

fn compile_return() -> String {
    "\
    pop rax
    leave
    ret\n"
        .to_string()
}

struct CommandContext<'rtm> {
    file_name: String,
    runtime: &'rtm Runtime,
}

impl<'rtm> CommandContext<'_> {
    fn statics_var_name(&self) -> String {
        format!("{}.statics", self.file_name)
    }
    fn get_address(&self, register: &'static str, index: usize) -> String {
        if self.runtime.emulate_hack_machine {
            format!("qword [RAM + {}*8 + {}]", register, index * 8)
        } else {
            format!("qword [{} + {}]", register, index * 8)
        }
    }
}

struct DataSection {
    name: String,
    table: HashMap<String, (String, String)>,
}

impl DataSection {
    fn new(name: &str) -> DataSection {
        DataSection {
            name: name.to_string(),
            table: HashMap::new(),
        }
    }
    fn insert(&mut self, name: &str, data_type: &str, data_expr: &str) -> Result<()> {
        match self.table.insert(
            name.to_string(),
            (data_type.to_string(), data_expr.to_string()),
        ) {
            None => Ok(()),
            Some(_) => Err(anyhow!("data with name {} already exists", name)),
        }
    }
    fn to_string(&self) -> String {
        let mut lines = format!("section .{}\n", self.name);
        for (key, val) in self.table.iter() {
            let (data_type, data_expr) = val;
            lines.push_str(&format!("    {:20} {:10} {}\n", key, data_type, data_expr));
        }
        lines.push_str("\n");
        lines
    }
}

pub fn compile_vm_to_asm(
    program: &TokenizedProgram,
    runtime: &Runtime,
    externs: &[&str],
    output_path: &Path,
) -> Result<()> {
    let mut output_file = fs::File::create(output_path).with_context(|| {
        format!(
            "Failed to create output file {}",
            output_path.to_string_lossy()
        )
    })?;
    let mut data_section = DataSection::new("data");
    data_section.insert("EXIT_SUCCESS", "equ", "0")?;
    data_section.insert("SYS_exit", "equ", "60")?;
    let mut bss_section = DataSection::new("bss");
    bss_section.insert("TEMP", "resq", &format!("{}", 5 * 8))?;
    if runtime.emulate_hack_machine {
        bss_section.insert("RAM", "resq", &format!("{}", (16384 + 8192 + 1) * 8))?;
    }

    let entry = match runtime.get_entry_point() {
        Some(entry_point) => format!(
            "
            ; Arguments Passed:
            ;     1) rdi - address of memory block
            ; Returns: VOID
            global {name}
            {name}:
                mov dword [rdi], 53
                mov dword [rdi], RAM
                call sys.init
                ret
                \n",
            name = entry_point
        ),
        None => "
global main
main:
    call sys.init
    ret
            \n"
        .to_string(),
    };

    let externs = externs
        .iter()
        .map(|e| format!("extern {}\n", e))
        .collect::<String>();

    let preamble = format!(
        "
{}
section .text
{}\n",
        externs, entry
    );
    let indent = |lines: String| -> String {
        lines
            .lines()
            .map(|line| format!("\t{}\n", line.trim()))
            .collect()
    };
    let mut lines: Vec<String> = vec![];
    for file in program.files.iter() {
        let mut num_statics = 0;
        let context = CommandContext {
            file_name: file.name.clone(),
            runtime,
        };
        for function in file.functions.iter() {
            for command in function.commands.iter() {
                let comment: String = format!("; {}", command);
                lines.push(comment);

                num_statics = match command {
                    VMToken::Push(VMSegment::Static, index)
                    | VMToken::Pop(VMSegment::Static, index) => cmp::max(num_statics, *index + 1),
                    _ => num_statics,
                };

                let asm = match command {
                    VMToken::Call(func_name, num_args) => {
                        compile_call(&func_name.to_lowercase(), num_args)
                    }
                    VMToken::Function(func_name, num_locals) => {
                        compile_function(&func_name.to_lowercase(), num_locals)
                    }
                    VMToken::Return => compile_return(),
                    VMToken::Push(segment, index) => compile_push(&context, segment, index),
                    VMToken::Pop(segment, index) => compile_pop(&context, segment, index),
                    VMToken::Neg
                    | VMToken::Not
                    | VMToken::Add
                    | VMToken::Sub
                    | VMToken::And
                    | VMToken::Or
                    | VMToken::Eq
                    | VMToken::Lt
                    | VMToken::Gt => compile_arithmetic(command),
                    VMToken::Label(label) => format!(".{}:", label),
                    VMToken::Goto(label) => format!("jmp .{}", label),
                    VMToken::If(label) => {
                        format!(
                            "\
                                pop      rax
                                cmp      rax, 0
                                jne .{}
                            ",
                            label
                        )
                    }
                    VMToken::None => "".to_string(),
                };
                let asm = indent(asm);
                lines.push(asm);
            }
        }
        if num_statics > 0 {
            bss_section.insert(
                &format!("{}.statics", file.name),
                "resq",
                &format!("{}", num_statics * 8),
            )?;
        }
        info!("Compiled {} to assembly", file.name);
    }

    output_file
        .write(data_section.to_string().as_bytes())
        .context("Failed writing data section to output file")?;
    output_file
        .write(bss_section.to_string().as_bytes())
        .context("Failed writing bss section to output file")?;
    output_file
        .write(preamble.as_bytes())
        .context("Failed writing preamble to output file")?;
    output_file
        .write(lines.join("\n").as_bytes())
        .context("Failed writing to output file")?;
    Ok(())
}
