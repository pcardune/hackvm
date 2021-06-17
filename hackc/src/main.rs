use anyhow::{anyhow, Context, Result};
use clap::{App, Arg};
use hackvm::{TokenizedProgram, VMSegment, VMToken};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{cmp, fs, process};

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
            format!("pop     qword [RAM + {}]", (index + 5) * 8)
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
        VMSegment::This => format!("pop qword [RAM + r14*8 + {}]", index * 8),
        VMSegment::That => format!("pop qword [RAM + r15*8 + {}]", index * 8),
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
        VMSegment::This => "push qword [RAM + r14*8]".to_string(),
        VMSegment::That => "push qword [RAM + r15*8]".to_string(),
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
                        push     qword [RAM + {}]\n",
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

fn compile_call(func_name: &str, num_args: &u16) -> String {
    let mut lines = String::new();
    // push the number of arguments onto the stack
    lines.push_str(&format!("mov rdx, {}\n", num_args));
    lines.push_str("push rdx\n");

    lines.push_str(&format!("call {}\n", func_name));
    for _ in 0..(*num_args + 1) {
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

struct CommandContext {
    file_name: String,
}

impl CommandContext {
    fn statics_var_name(&self) -> String {
        format!("{}.statics", self.file_name)
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

fn compile(program: &TokenizedProgram, output_path: &Path) -> Result<()> {
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
    bss_section.insert("RAM", "resq", &format!("{}", (16384 + 8192 + 1) * 8))?;
    let preamble = "
section .text

; Arguments Passed:
;     1) rdi - address of memory block
; Returns: VOID
global hack_sys_init
hack_sys_init:
    mov dword [rdi], 53
    mov dword [rdi], RAM
    call Sys.init
    ret
    \n";
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
                    VMToken::Call(func_name, num_args) => compile_call(func_name, num_args),
                    VMToken::Function(func_name, num_locals) => {
                        compile_function(func_name, num_locals)
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

fn run<'a>(prefix: &str, command: &'a mut Command) -> bool {
    let prefix = format!("[{}]", prefix);
    let mut child = command
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = BufReader::new(child.stderr.take().unwrap());
    let stdout = BufReader::new(child.stdout.take().unwrap());

    let add_prefix = |line: String| println!("{} {}", prefix, line);
    stdout
        .lines()
        .filter_map(|line| line.ok())
        .for_each(add_prefix);
    stderr
        .lines()
        .filter_map(|line| line.ok())
        .for_each(add_prefix);
    let exit_status = child.wait().expect("command wasn't running");
    exit_status.success()
}

/// Traverses a directory to get a vector of paths to .vm files
fn get_program_file_paths(path: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let mut paths: Vec<std::path::PathBuf> = vec![];
    if path.is_file() {
        paths.push(path.to_path_buf());
    } else {
        for entry in std::fs::read_dir(path).with_context(|| "Failed reading directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "vm" {
                        paths.push(path);
                    }
                }
            }
        }
    }
    return Ok(paths);
}

fn get_tokenized_program(path: &std::path::Path) -> Result<TokenizedProgram> {
    let paths = get_program_file_paths(path)?;
    let mut files: Vec<(String, String)> = vec![];
    for path in paths.iter() {
        let file_name = path
            .file_name()
            .expect("paths to be filtered for proper files already")
            .to_string_lossy()
            .to_string();
        let file_content = fs::read_to_string(path).with_context(|| "Failed to read file")?;
        files.push((file_name, file_content));
    }
    let mut stuff: Vec<(&str, &str)> = vec![];
    for (file_name, file_content) in files.iter() {
        stuff.push((&file_name, &file_content));
    }
    TokenizedProgram::from_files(&stuff).map_err(|e| anyhow::format_err!(e))
}

fn main() {
    let matches = App::new("hackc")
        .arg(
            Arg::with_name("input")
                .required(true)
                .short("i")
                .takes_value(true),
        )
        .get_matches();
    let input_file_path = matches.value_of("input").unwrap();
    let tokenized_program = get_tokenized_program(std::path::Path::new(input_file_path)).unwrap();

    let out_dir = std::path::Path::new("out");
    fs::create_dir_all(out_dir).unwrap();

    let asm_out_path = out_dir.join("out.asm");
    compile(&tokenized_program, &asm_out_path).unwrap();

    let obj_out_path = out_dir.join("out.o");
    let list_out_path = out_dir.join("out.lst");
    let executable_out_path = out_dir.join("out");
    if !run(
        "assemble",
        Command::new("yasm")
            .arg("-Werror")
            .arg("-Worphan-labels")
            .arg("-g")
            .arg("dwarf2")
            .arg("-f")
            .arg("elf64")
            .arg(&asm_out_path)
            .arg("-o")
            .arg(&obj_out_path)
            .arg("-l")
            .arg(&list_out_path),
    ) {
        println!("Well that didn't go well...");
        process::exit(1);
    }

    let runtime_dir = std::path::Path::new("hackc/runtime");
    let runtime_obj_path = out_dir.join("runtime.o");
    if !run(
        "runtime",
        Command::new("g++")
            .arg("-g")
            .arg("-Wall")
            .arg("-c")
            .arg(runtime_dir.join("main.cpp"))
            .arg("-o")
            .arg(&runtime_obj_path),
    ) {
        println!("Well that didn't go well...");
        process::exit(1);
    }

    if !run(
        "link",
        process::Command::new("g++")
            .arg(&runtime_obj_path)
            .arg(&obj_out_path)
            .arg("-std=c++11")
            .arg("-pthread")
            .arg("-g")
            .arg("-no-pie")
            .arg("-lSDL2")
            .arg("-o")
            .arg(&executable_out_path),
    ) {
        println!("Well that didn't go well...");
        process::exit(1);
    }
}
