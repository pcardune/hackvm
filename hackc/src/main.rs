use anyhow::{anyhow, Context, Result};
use clap::{App, Arg};
use hackvm::{TokenizedProgram, VMSegment, VMToken};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
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
        VMSegment::This => format!("pop qword [RAM + r14*8 + {}*8]", index),
        VMSegment::That => format!("pop qword [RAM + r15*8 + {}*8]", index),
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
        VMSegment::This => format!("push qword [RAM + r14*8 + {}*8]", index),
        VMSegment::That => format!("push qword [RAM + r15*8 + {}*8]", index),
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

fn compile_vm_to_asm(program: &TokenizedProgram, output_path: &Path) -> Result<()> {
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

fn assemble(out_dir: &Path, asm_out_path: &Path) -> Result<PathBuf> {
    let obj_out_path = out_dir.join("out.o");
    let list_out_path = out_dir.join("out.lst");
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
        return Err(anyhow!(
            "Failed to assemble {}",
            asm_out_path.to_string_lossy()
        ));
    }
    return Ok(obj_out_path);
}

struct Runtime {
    cpp_file: PathBuf,
}

impl Runtime {
    fn default() -> Runtime {
        Runtime {
            cpp_file: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime/main.cpp"),
        }
    }

    fn debug() -> Runtime {
        Runtime {
            cpp_file: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime/debug.cpp"),
        }
    }

    fn compile(&self, out_dir: &Path) -> Result<PathBuf> {
        let runtime_obj_path = out_dir.join("runtime.o");
        let mut command = Command::new("g++");
        command
            .arg("-g")
            .arg("-Wall")
            .arg("-c")
            .arg(&self.cpp_file)
            .arg("-o")
            .arg(&runtime_obj_path);

        if !run("runtime", &mut command) {
            println!("Well that didn't go well...");
            return Err(anyhow!(
                "Failed to compile runtime {} with command {:?}",
                self.cpp_file.to_string_lossy(),
                command
            ));
        }
        return Ok(runtime_obj_path);
    }
}

fn link_executable(
    out_dir: &Path,
    runtime_obj_path: &Path,
    obj_out_path: &Path,
) -> Result<PathBuf> {
    let executable_out_path = out_dir.join("out");

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
        return Err(anyhow!("Failed to link executable"));
    }
    return Ok(executable_out_path);
}

struct Executable {
    input_path: PathBuf,
    out_dir: PathBuf,
    runtime: Runtime,
}

impl Executable {
    fn new(input_path: &Path, out_dir: &Path) -> Executable {
        Executable {
            input_path: input_path.to_path_buf(),
            out_dir: out_dir.to_path_buf(),
            runtime: Runtime::default(),
        }
    }

    fn runtime(mut self, runtime: Runtime) -> Executable {
        self.runtime = runtime;
        self
    }

    fn compile(&self) -> Result<PathBuf> {
        let tokenized_program = get_tokenized_program(&self.input_path).with_context(|| {
            format!(
                "Failed tokenizing program {}",
                self.input_path.to_string_lossy()
            )
        })?;

        let asm_out_path = self.out_dir.join("out.asm");
        compile_vm_to_asm(&tokenized_program, &asm_out_path)
            .with_context(|| "Failed compiling vmcode to asm")?;

        let obj_out_path = assemble(&self.out_dir, &asm_out_path)?;
        let runtime_obj_path = self.runtime.compile(&self.out_dir)?;

        return link_executable(&self.out_dir, &runtime_obj_path, &obj_out_path);
    }
}

fn exec_vm(executable_path: &Path) -> Result<bool> {
    let mut child = process::Command::new(executable_path).spawn()?;
    let exit_status = child.wait()?;
    Ok(exit_status.success())
}

fn main() {
    let matches = App::new("hackc")
        .arg(
            Arg::with_name("input")
                .required(true)
                .short("i")
                .takes_value(true),
        )
        .arg(Arg::with_name("exec").long("exec"))
        .get_matches();
    let input_file_path = matches.value_of("input").unwrap();
    let out_dir = Path::new("out");

    fs::create_dir_all(out_dir).unwrap();
    let executable_path = Executable::new(Path::new(input_file_path), out_dir)
        .runtime(Runtime::debug())
        .compile()
        .unwrap();

    if matches.is_present("exec") {
        exec_vm(&executable_path).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::hash::Hash;

    struct TestCaseResult<'a> {
        test_case: TestCase<'a>,
        interpreter_result: Result<(i32, hackvm::VMEmulatorRAM)>,
        compiled_result: Result<(Option<i32>, Vec<i64>)>,
    }

    impl<'a> TestCaseResult<'a> {
        fn compiled_return(&self) -> i32 {
            self.compiled_result.as_ref().unwrap().0.unwrap()
        }
        fn compiled_ram(&self) -> &[i64] {
            &self.compiled_result.as_ref().unwrap().1
        }
        fn interpreter_return(&self) -> i32 {
            self.interpreter_result.as_ref().unwrap().0
        }
        fn interpreter_ram(&self) -> &hackvm::VMEmulatorRAM {
            &self.interpreter_result.as_ref().unwrap().1
        }
        fn assert_return_eq(&self, expected: i32) -> &TestCaseResult {
            let code = self.compiled_return();
            if code != self.interpreter_return() {
                panic!(
                    "Interpreted and compiled return codes don't agree: {} != {}",
                    self.interpreter_return(),
                    code
                );
            } else if code != expected {
                panic!(
                    "Interpreted and compiled return codes were unexpected: {} != {}",
                    self.interpreter_return(),
                    expected
                );
            }
            self
        }
        fn assert_ram_eq(&self, start: usize, end: usize) -> &TestCaseResult {
            let mut failures: Vec<usize> = vec![];
            for i in start..end {
                if self.interpreter_ram()[i] != self.compiled_ram()[i] as i32 {
                    failures.push(i);
                }
            }
            if failures.len() > 0 {
                let message = failures
                    .into_iter()
                    .map(|i| {
                        format!(
                            "  [{}] {} != {}",
                            i,
                            self.interpreter_ram()[i],
                            self.compiled_ram()[i]
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                panic!("Interpreted and compiled results don't agree:\n{}", message);
            }
            self
        }
    }

    struct TestCase<'a> {
        files: Vec<(&'a str, &'a str)>,
        max_steps: usize,
        ram_size: usize,
    }
    impl<'a> TestCase<'a> {
        fn with_code(code: &'a str) -> TestCase<'a> {
            TestCase {
                files: vec![("Sys.vm", code)],
                max_steps: 1000,
                ram_size: 20,
            }
        }

        fn ram_size(mut self, size: usize) -> TestCase<'a> {
            self.ram_size = size;
            self
        }

        fn run(self) -> TestCaseResult<'a> {
            let interpreter_result = self.run_interpreter();
            let compiled_result = self.run_compiled();
            TestCaseResult {
                test_case: self,
                interpreter_result,
                compiled_result,
            }
        }

        fn run_interpreter(&self) -> Result<(i32, hackvm::VMEmulatorRAM)> {
            let program = hackvm::VMProgram::new(&self.files).unwrap();
            let mut vm = hackvm::VMEmulator::new(program);
            let return_code = vm.run(self.max_steps).map_err(|e| anyhow!(e))?;
            Ok((return_code, vm.into_ram()))
        }

        fn get_hash_string(&self) -> String {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::Hasher;
            let mut state = DefaultHasher::new();
            for (name, code) in self.files.iter() {
                name.hash(&mut state);
                code.hash(&mut state);
            }
            let hash = state.finish();
            hash.to_string()
        }

        fn run_compiled(&self) -> Result<(Option<i32>, Vec<i64>)> {
            let out_dir = Path::new("/tmp/hackc").join(self.get_hash_string());
            let program_dir = out_dir.join("program");
            fs::create_dir_all(&program_dir).with_context(|| "Failed creating temp directory")?;
            for (name, code) in self.files.iter() {
                fs::write(program_dir.join(name), code)
                    .with_context(|| "Failed writing vmcode to disk")?;
            }
            let executable_path = Executable::new(&program_dir, &out_dir)
                .runtime(Runtime::debug())
                .compile()
                .with_context(|| {
                    format!(
                        "Failed compiling executable for {}",
                        program_dir.to_string_lossy()
                    )
                })?;
            let output = process::Command::new(&executable_path)
                .arg("0")
                .arg(self.ram_size.to_string())
                .output()
                .with_context(|| {
                    format!(
                        "Failed spawning executable {}",
                        executable_path.to_string_lossy()
                    )
                })?;
            let stdout = String::from_utf8(output.stdout)
                .with_context(|| "Failed converting output to utf8")?;
            let mut ram: Vec<i64> = vec![-1; self.ram_size];
            stdout.lines().for_each(|line| {
                let mut parts = line.split(":");
                if let Some(index) = parts.next() {
                    if let Some(value) = parts.next() {
                        if let Ok(index) = index.parse::<usize>() {
                            if let Ok(value) = value.parse() {
                                ram[index] = value;
                            }
                        }
                    }
                }
            });
            // Ok((output.status.code(), ram))
            Ok((Some(ram[0] as i32), ram))
        }
    }

    #[test]
    #[serial]
    fn test_temp_segment() {
        let code = "
            function Sys.init 0
                push constant 10
                pop temp 0
                push constant 12
                pop temp 7
                push temp 7
            return
        ";
        let result = TestCase::with_code(code).run();
        result.assert_ram_eq(5, 5 + 8);
        result.assert_return_eq(12);
        assert_eq!(result.compiled_ram()[5], 10);
    }

    #[test]
    #[serial]
    fn test_this_that_pointer_segments() {
        TestCase::with_code(
            "
            function Sys.init 0
                push constant 1000
                pop pointer 0
                push constant 1050
                pop pointer 1
                push constant 3
                pop this 0
                push constant 5
                pop this 1
                push constant 2
                pop that 0
                push constant 4
                pop that 1
                push constant 0
            return
        ",
        )
        .ram_size(2000)
        .run()
        .assert_ram_eq(1000, 1003)
        .assert_ram_eq(1050, 1060);
    }

    fn test_arithmetic(a: u32, b: u32, op: &str, expected: i32) {
        TestCase::with_code(&format!(
            "
            function Sys.init 0
            push constant {}
            push constant {}
            {}
            return
        ",
            a, b, op
        ))
        .run()
        .assert_return_eq(expected);
    }

    #[test]
    #[serial]
    fn test_add() {
        test_arithmetic(10, 12, "add", 22);
    }

    #[test]
    #[serial]
    fn test_sub() {
        test_arithmetic(7, 9, "sub", -2);
    }

    #[test]
    #[serial]
    fn test_neg() {
        test_arithmetic(7, 8, "neg", -8);
    }

    #[test]
    #[serial]
    fn test_not() {
        test_arithmetic(7, 5, "not", -6);
    }

    #[test]
    #[serial]
    fn test_and() {
        test_arithmetic(21, 25, "and", 17);
    }

    #[test]
    #[serial]
    fn test_or() {
        test_arithmetic(21, 25, "or", 29);
    }

    #[test]
    #[serial]
    fn test_eq() {
        test_arithmetic(21, 23, "eq", 0);
        test_arithmetic(21, 21, "eq", -1);
    }

    #[test]
    #[serial]
    fn test_lt() {
        test_arithmetic(21, 23, "lt", -1);
        test_arithmetic(23, 21, "lt", 0);
    }

    #[test]
    #[serial]
    fn test_gt() {
        test_arithmetic(21, 23, "gt", 0);
        test_arithmetic(23, 21, "gt", -1);
    }
}
