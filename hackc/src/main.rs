use anyhow::{Context, Result};
use clap::{App, Arg};
use hackvm::{TokenizedProgram, VMSegment, VMToken};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{fs, process};

fn compile_arithmetic(op: &VMToken) -> String {
    match op {
        VMToken::Add => "\
            pop     rax
            pop     rbx
            add     rax, rbx
            push    rax"
            .to_string(),
        VMToken::Sub => "\
            pop     rax
            pop     rbx
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
            pop     rax
            pop     rbx
            cmp     rax, rbx
            mov     rcx, -1
            cmovg   rax, rcx
            mov     rcx, 0
            cmovng  rax, rcx
            push    rax"
            .to_string(),
        VMToken::Lt => "\
            pop     rax
            pop     rbx
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

fn compile_pop(segment: &VMSegment, index: &u16) -> String {
    match segment {
        VMSegment::Temp => {
            format!("pop     qword [RAM + {}]", (index + 5) * 8)
        }
        _ => panic!("Don't know how to pop to segment {:?} yet", segment),
    }
}

fn compile_push(segment: &VMSegment, index: &u16) -> String {
    match segment {
        VMSegment::Constant => {
            let value = index;
            format!(
                "\
                        mov      rax, {}\n\
                        push     rax\n",
                value
            )
        }
        VMSegment::Temp => {
            format!(
                "\
                        push     qword [RAM + {}]\n",
                (index + 5) * 8
            )
        }
        _ => panic!(
            "Don't know how to compile push for segment {:?} yet",
            segment
        ),
    }
}

fn compile(program: &TokenizedProgram, output_path: &Path) -> Result<()> {
    let mut file = fs::File::create(output_path).with_context(|| {
        format!(
            "Failed to create output file {}",
            output_path.to_string_lossy()
        )
    })?;
    let preamble = "
section .data
    EXIT_SUCCESS    equ     0
    SYS_exit        equ     60

section .bss
    RAM             resq    16384 + 8192 + 1

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
    write!(file, "{}", preamble).context("Failed writing to output file")?;
    let indent = |lines: String| -> String {
        lines
            .lines()
            .map(|line| format!("\t{}\n", line.trim()))
            .collect()
    };
    for command in program.files[0].functions[0].commands.iter() {
        let asm = match command {
            VMToken::Function(func_name, _num_locals) => {
                format!("global {}\n{}:\n", func_name, func_name)
            }
            VMToken::Return => "\
                pop      rax\n\
                ret\n"
                .to_string(),
            VMToken::Push(segment, index) => compile_push(segment, index),
            VMToken::Pop(segment, index) => compile_pop(segment, index),
            VMToken::Neg
            | VMToken::Not
            | VMToken::Add
            | VMToken::Sub
            | VMToken::And
            | VMToken::Or
            | VMToken::Eq
            | VMToken::Lt
            | VMToken::Gt => compile_arithmetic(command),
            _ => panic!("Don't know how to compile {:?} yet", command),
        };
        let asm = indent(asm);
        let comment: String = format!("; {}", command);
        write!(file, "{}\n{}", comment, asm).context("failed writing to output file")?;
    }
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

    let runtime_dir = std::path::Path::new("hackc/runtime");
    let runtime_obj_path = out_dir.join("runtime.o");
    if !run(
        "runtime",
        Command::new("gcc")
            .arg("-g")
            .arg("-Wall")
            .arg("-c")
            .arg(runtime_dir.join("main.c"))
            .arg("-o")
            .arg(&runtime_obj_path),
    ) {
        println!("Well that didn't go well...");
        process::exit(1);
    }

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

    if !run(
        "link",
        process::Command::new("g++")
            .arg("-g")
            .arg("-no-pie")
            .arg("-o")
            .arg(&executable_out_path)
            .arg(&runtime_obj_path)
            .arg(&obj_out_path),
    ) {
        println!("Well that didn't go well...");
        process::exit(1);
    }
}
