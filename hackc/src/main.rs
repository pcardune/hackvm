use anyhow::{anyhow, Context, Result};
use clap::{App, Arg};
use hackvm::TokenizedProgram;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{fs, process};
mod codegen;

use codegen::compile_vm_to_asm;

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

fn get_tokenized_program(
    paths: &[std::path::PathBuf],
    include_os: bool,
) -> Result<TokenizedProgram> {
    let mut files: HashMap<String, String> = HashMap::new();

    for path in paths.iter() {
        let file_name = path
            .file_name()
            .expect("paths to be filtered for proper files already")
            .to_string_lossy()
            .to_string();
        let file_content = fs::read_to_string(path).with_context(|| "Failed to read file")?;
        files.insert(file_name, file_content);
    }

    #[rustfmt::skip]
    let os_files = vec![
        ("Array.vm",    std::include_str!("../examples/vmcode/OS/Array.vm")),
        ("Keyboard.vm", std::include_str!("../examples/vmcode/OS/Keyboard.vm")),
        ("Math.vm",     std::include_str!("../examples/vmcode/OS/Math.vm")),
        ("Memory.vm",   std::include_str!("../examples/vmcode/OS/Memory.vm")),
        ("Output.vm",   std::include_str!("../examples/vmcode/OS/Output.vm")),
        ("Screen.vm",   std::include_str!("../examples/vmcode/OS/Screen.vm")),
        ("String.vm",   std::include_str!("../examples/vmcode/OS/String.vm")),
        ("Sys.vm",      std::include_str!("../examples/vmcode/OS/Sys.vm")),
    ];
    if include_os {
        for os_file in os_files {
            if !files.contains_key(os_file.0) {
                files.insert(os_file.0.to_string(), os_file.1.to_string());
            }
        }
    }
    let file_refs = files
        .iter()
        .map(|f| (f.0.as_str(), f.1.as_str()))
        .collect::<Vec<_>>();
    TokenizedProgram::from_files(&file_refs).map_err(|e| anyhow::format_err!(e))
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

    #[cfg(test)]
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
    include_os: bool,
}

impl Executable {
    fn new(input_path: &Path, out_dir: &Path) -> Executable {
        Executable {
            input_path: input_path.to_path_buf(),
            out_dir: out_dir.to_path_buf(),
            runtime: Runtime::default(),
            include_os: false,
        }
    }

    fn runtime(mut self, runtime: Runtime) -> Executable {
        self.runtime = runtime;
        self
    }

    fn include_os(mut self, include_os: bool) -> Executable {
        self.include_os = include_os;
        self
    }

    fn compile(&self) -> Result<PathBuf> {
        let paths = get_program_file_paths(&self.input_path)?;

        let tokenized_program =
            get_tokenized_program(&paths, self.include_os).with_context(|| {
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
        .runtime(Runtime::default())
        .include_os(true)
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

    struct TestCaseResult {
        interpreter_result: Result<(i32, hackvm::VMEmulatorRAM)>,
        compiled_result: Result<(Option<i32>, Vec<i64>)>,
    }

    impl TestCaseResult {
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
        fn with_files(files: Vec<(&'a str, &'a str)>) -> TestCase<'a> {
            TestCase {
                files,
                max_steps: 1000,
                ram_size: 20,
            }
        }
        fn with_code(code: &'a str) -> TestCase<'a> {
            Self::with_files(vec![("Sys.vm", code)])
        }

        fn ram_size(mut self, size: usize) -> TestCase<'a> {
            self.ram_size = size;
            self
        }

        fn run(self) -> TestCaseResult {
            let interpreter_result = self.run_interpreter();
            let compiled_result = self.run_compiled();
            TestCaseResult {
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

    mod segments {
        use super::*;
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

        #[test]
        #[serial]
        fn test_this_that_pointer_segments_across_function_calls() {
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
                    pop that 0
                    call Sys.next 0
                    pop temp 0
                    push this 0
                    pop temp 1
                    push that 0
                    pop temp 2
                    push constant 0
                return

                function Sys.next 0
                    push constant 1100
                    pop pointer 0
                    push constant 1150
                    pop pointer 1
                    push constant 7
                    pop this 0
                    push constant 8
                    pop that 0
                    push constant 0
                return
                ",
            )
            .ram_size(1500)
            .run()
            .assert_ram_eq(5, 5 + 8);
        }

        #[test]
        #[serial]
        fn test_static_segment_across_functions() {
            // The static segment is shared across
            // all functions within a given file
            TestCase::with_code(
                "
                function Sys.init 0
                    push constant 10
                    pop static 0
                    call Sys.changeStatic 0
                    pop temp 0
                    push static 0
                return

                function Sys.changeStatic 0
                    push constant 12
                    pop static 0
                    push constant 0
                return
                ",
            )
            .run()
            .assert_return_eq(12);
        }

        #[test]
        #[serial]
        fn test_static_segment_across_files() {
            // Each file gets its own static segment
            TestCase::with_files(vec![
                (
                    "Sys.vm",
                    "
                    function Sys.init 0
                        push constant 10
                        pop static 0
                        call Other.changeStatic 0
                        pop temp 0
                        push static 0
                    return
                    ",
                ),
                (
                    "Other.vm",
                    "
                    function Other.changeStatic 0
                        push constant 12
                        pop static 0
                        push constant 0
                    return
                    ",
                ),
            ])
            .run()
            .assert_return_eq(10);
        }

        #[test]
        #[serial]
        fn test_local_segment_across_functions() {
            // Each function gets its own local segment,
            // so the changes made in changeAnotherLocal
            // should not affect the local from changeLocal
            TestCase::with_code(
                "
                function Sys.init 0
                    call Sys.changeLocal 0
                return

                function Sys.changeLocal 1
                    push constant 10
                    pop local 0
                    call Sys.changeAnotherLocal 0
                    pop temp 0
                    push local 0
                return

                function Sys.changeAnotherLocal 1
                    push constant 12
                    pop local 0
                    push constant 0
                return
                ",
            )
            .run()
            .assert_return_eq(10);
        }

        #[test]
        #[serial]
        fn test_argument_segment() {
            TestCase::with_code(
                "
                function Sys.init 0
                    push constant 10
                    push constant 12
                    call Sys.subArgs 2
                return

                function Sys.subArgs 0
                    push argument 0
                    push argument 1
                    sub
                return
                ",
            )
            .run()
            .assert_return_eq(-2);
        }
    }

    mod branching {
        use super::*;
        #[test]
        #[serial]
        fn test_goto() {
            // goto should unconditionally jump to the specified label,
            // skipping whatever code is inbetween
            TestCase::with_code(
                "
                function Sys.init 0
                    push constant 10
                    goto THE_END
                    push constant 12
                    label THE_END
                    push constant 5
                    add
                return
                ",
            )
            .run()
            .assert_return_eq(15);
        }

        #[test]
        #[serial]
        fn test_labels_are_function_scoped() {
            // labels are scoped to the function they are defined in,
            // so goto THE_END in Sys.init goes to label THE_END in Sys.init
            // and not in Sys.other
            TestCase::with_code(
                "
                function Sys.init 0
                    push constant 10
                    goto THE_END
                    push constant 12
                    label THE_END
                    push constant 5
                    add
                    call Sys.other 0
                    add
                return

                function Sys.other 0
                    push constant 20
                    goto THE_END
                    push constant 32
                    label THE_END
                    push constant 15
                    add
                return
                ",
            )
            .run()
            .assert_return_eq(50);
        }

        #[test]
        #[serial]
        fn test_if_goto() {
            let code = |cond: &str| {
                format!(
                    "
                function Sys.init 0
                    push constant 100
                    push constant 10
                    push constant 12
                    {}
                    if-goto THE_END
                    push constant 12
                    label THE_END
                    push constant 5
                    add
                return
                ",
                    cond
                )
            };
            TestCase::with_code(&code("gt")).run().assert_return_eq(17);
            TestCase::with_code(&code("lt")).run().assert_return_eq(105);
        }
    }

    mod arithmetic {
        use super::*;
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
        fn test_arithmetic_add() {
            test_arithmetic(10, 12, "add", 22);
        }

        #[test]
        #[serial]
        fn test_arithmetic_sub() {
            test_arithmetic(7, 9, "sub", -2);
        }

        #[test]
        #[serial]
        fn test_arithmetic_neg() {
            test_arithmetic(7, 8, "neg", -8);
        }

        #[test]
        #[serial]
        fn test_arithmetic_not() {
            test_arithmetic(7, 5, "not", -6);
        }

        #[test]
        #[serial]
        fn test_arithmetic_and() {
            test_arithmetic(21, 25, "and", 17);
        }

        #[test]
        #[serial]
        fn test_arithmetic_or() {
            test_arithmetic(21, 25, "or", 29);
        }

        #[test]
        #[serial]
        fn test_arithmetic_eq() {
            test_arithmetic(21, 23, "eq", 0);
            test_arithmetic(21, 21, "eq", -1);
        }

        #[test]
        #[serial]
        fn test_arithmetic_lt() {
            test_arithmetic(21, 23, "lt", -1);
            test_arithmetic(23, 21, "lt", 0);
        }

        #[test]
        #[serial]
        fn test_arithmetic_gt() {
            test_arithmetic(21, 23, "gt", 0);
            test_arithmetic(23, 21, "gt", -1);
        }
    }
}
