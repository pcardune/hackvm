use anyhow::{Context, Result};
use clap::{App, Arg};
use hackvm::{VMCommand, VMOperation, VMProgram, VMSegment};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{fs, process};

fn compile(program: &VMProgram, output_path: &Path) -> Result<()> {
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
    let indent =
        |lines: String| -> String { lines.lines().map(|line| format!("\t{}\n", line)).collect() };
    for command in program.files[0].functions[0].commands.iter() {
        let asm = match command {
            VMCommand::Function(func_ref, _num_locals) => {
                let func_name = program.get_function_name(func_ref).unwrap();
                format!("global {}\n{}:\n", func_name, func_name)
            }
            VMCommand::Return => "\
                pop      ax\n\
                ret\n"
                .to_string(),
            VMCommand::Push(segment, index) => match segment {
                VMSegment::Constant => {
                    let value = index;
                    format!(
                        "\
                        mov      ax, {}\n\
                        push     ax\n",
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
                _ => panic!("Don't know how to compile {:?} yet", command),
            },
            VMCommand::Pop(segment, index) => match segment {
                VMSegment::Temp => {
                    format!(
                        "\
                        pop     qword [RAM + {}]
                        ",
                        (index + 5) * 8
                    )
                }
                _ => panic!("Don't know how to pop to segment {:?} yet", segment),
            },
            VMCommand::Arithmetic(op) => match op {
                VMOperation::Add => {
                    format!(
                        "\
                        pop      ax\n\
                        pop      bx\n\
                        add      ax, bx\n\
                        push     ax\n"
                    )
                }
                _ => panic!("Don't know how to compile {:?} yet", command),
            },
            _ => panic!("Don't know how to compile {:?} yet", command),
        };
        let asm = indent(asm);
        write!(file, "; {}\n{}", command.to_string(program), asm)
            .context("failed writing to output file")?;
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

fn load_vm_program(path: &str) -> Result<VMProgram, String> {
    let file_content = fs::read_to_string(path).unwrap();
    let filename = std::path::Path::new(path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let files = vec![(filename, &file_content[..])];
    VMProgram::new(&files)
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
    let vm_program = match load_vm_program(input_file_path) {
        Err(msg) => {
            println!("Failed loading program: {}", msg);
            process::exit(1);
        }
        Ok(vm_program) => {
            println!("Successfully loaded program {}", input_file_path);
            vm_program
        }
    };

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
    compile(&vm_program, &asm_out_path).unwrap();

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
