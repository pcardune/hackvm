use super::vmcommand::{
    Command, FunctionRef, InCodeFuncRef, Operation, Segment, VMProgram, SEGMENTS,
};
use std::collections::HashMap;
use std::convert::TryInto;
use std::{cmp, usize};

const RAM_SIZE: usize = 16384 + 8192 + 1;

#[derive(Debug)]
struct VMStackFrame {
    local_segment: Vec<i32>,
    stack_size: usize,
    function: InCodeFuncRef,
    index: usize,
    num_args: usize,
}

impl VMStackFrame {
    fn new(function: InCodeFuncRef, num_args: usize) -> VMStackFrame {
        VMStackFrame {
            local_segment: Vec::new(),
            stack_size: 0,
            function,
            index: 0,
            num_args,
        }
    }
}

struct VMProfileFuncStats {
    num_calls: u64,
    num_steps: u64,
}

struct VMProfiler {
    function_stats: HashMap<FunctionRef, VMProfileFuncStats>,
}

impl VMProfiler {
    pub fn new() -> VMProfiler {
        VMProfiler {
            function_stats: HashMap::new(),
        }
    }

    fn add_function_stats(&mut self, func_ref: FunctionRef, func_stats: VMProfileFuncStats) {
        match self.function_stats.get_mut(&func_ref) {
            Some(stats) => {
                stats.num_calls += func_stats.num_calls;
                stats.num_steps += func_stats.num_steps;
            }
            None => {
                self.function_stats.insert(func_ref, func_stats);
            }
        }
    }

    pub fn count_function_step(&mut self, func_ref: FunctionRef) {
        self.add_function_stats(
            func_ref,
            VMProfileFuncStats {
                num_calls: 0,
                num_steps: 1,
            },
        );
    }

    pub fn count_function_call(&mut self, func_ref: FunctionRef) {
        self.add_function_stats(
            func_ref,
            VMProfileFuncStats {
                num_calls: 1,
                num_steps: 0,
            },
        );
    }
}

pub struct VMEmulator {
    program: VMProgram,
    ram: [i32; RAM_SIZE],
    call_stack: Vec<VMStackFrame>,
    step_counter: usize,
    profiler: VMProfiler,
}

const SP: usize = 0;
const LCL: usize = 1;
const ARG: usize = 2;
const THIS: usize = 3;
const THAT: usize = 4;

impl VMEmulator {
    pub fn empty() -> VMEmulator {
        VMEmulator {
            program: VMProgram::empty(),
            ram: [0; RAM_SIZE],
            call_stack: Vec::new(),
            step_counter: 0,
            profiler: VMProfiler::new(),
        }
    }
    pub fn new(program: VMProgram) -> VMEmulator {
        VMEmulator {
            program,
            ram: [0; RAM_SIZE],
            call_stack: Vec::new(),
            step_counter: 0,
            profiler: VMProfiler::new(),
        }
    }

    fn frame_mut(&mut self) -> &mut VMStackFrame {
        self.call_stack.last_mut().expect("call stack is empty")
    }

    fn frame(&self) -> &VMStackFrame {
        self.call_stack.last().expect("call stack is empty")
    }

    pub fn ram(&self) -> &[i32] {
        &self.ram
    }

    pub fn reset(&mut self) {
        self.ram = [0; RAM_SIZE];
        self.call_stack = Vec::new();
        self.step_counter = 0;
        self.init().unwrap();
    }

    pub fn set_ram(&mut self, address: usize, value: i32) -> Result<(), &'static str> {
        if address < RAM_SIZE {
            self.ram[address] = value;
            return Ok(());
        }
        return Err("Address out of range");
    }

    pub fn get_ram_range(&self, start: usize, end: usize) -> &[i32] {
        &self.ram[start..end]
    }

    fn get_global_stack_bounds(&self) -> (usize, usize) {
        (256, self.ram[SP] as usize)
    }

    fn get_global_stack(&self) -> &[i32] {
        let (start, end) = self.get_global_stack_bounds();
        &self.ram[start..end]
    }

    fn get_global_stack_mut(&mut self) -> &mut [i32] {
        let (start, end) = self.get_global_stack_bounds();
        &mut self.ram[start..end]
    }
    fn push_global_stack(&mut self, value: i32) {
        self.ram[SP] += 1;
        if let Some(last) = self.get_global_stack_mut().last_mut() {
            *last = value;
        }
    }

    fn pop_global_stack(&mut self) -> Result<i32, String> {
        if self.ram[SP] <= 256 {
            return Err("Global stack is empty".to_string());
        }
        self.ram[SP] -= 1;
        return Ok(self.ram[self.ram[SP] as usize]);
    }

    fn pop_stack(&mut self) -> Result<i32, String> {
        if self.frame().stack_size > 0 {
            self.frame_mut().stack_size -= 1;
            return self.pop_global_stack();
        }
        return Err("local stack is empty".to_string());
    }

    fn push_stack(&mut self, value: i32) {
        self.frame_mut().stack_size += 1;
        self.push_global_stack(value);
    }

    fn get_stack(&self) -> &[i32] {
        let (_, local_end) = self.get_segment_bounds(Segment::Local);
        let start = cmp::max(local_end, 256);
        let end = self.ram[SP] as usize;
        &self.ram[start..end]
    }

    fn peek_stack(&self) -> i32 {
        *self.get_stack().last().expect("Stack is empty")
    }

    fn get_segment_bounds(&self, segment: Segment) -> (usize, usize) {
        match segment {
            Segment::Static => {
                let function = &self.frame().function;
                let vmfile = self.program.get_file(function);
                let start = 16 + vmfile.static_offset;
                let end = start + vmfile.num_statics;
                (start, end)
            }
            Segment::Pointer => (THIS, THAT + 1),
            Segment::Temp => (5, 5 + 8),
            Segment::This => (self.ram[THIS] as usize, self.ram.len()),
            Segment::That => (self.ram[THAT] as usize, self.ram.len()),
            Segment::Local => {
                let start = self.ram[LCL] as usize;
                let num_locals = self
                    .program
                    .get_vmfunction(&self.frame().function)
                    .num_locals;
                let end = start + num_locals;
                (start, end)
            }
            Segment::Argument => {
                let start: usize = self.ram[ARG].try_into().unwrap();
                let end = start + self.frame().num_args;
                (start, end)
            }
            Segment::Constant => panic!("constant segment doesn't actually exist"),
        }
    }
    fn get_segment(&self, segment: Segment) -> &[i32] {
        let (start, end) = self.get_segment_bounds(segment);
        &self.ram[start..end]
    }

    fn get_segment_mut(&mut self, segment: Segment) -> &mut [i32] {
        let (start, end) = self.get_segment_bounds(segment);
        &mut self.ram[start..end]
    }

    fn exec_push(&mut self, segment: Segment, index: u16) -> Result<(), String> {
        // TODO: check boundaries
        let value: i32 = match segment {
            Segment::Constant => index as i32,
            _ => self.get_segment(segment)[index as usize],
        };
        self.push_stack(value);
        Ok(())
    }

    fn exec_pop(&mut self, segment: Segment, index: u16) -> Result<(), String> {
        let value = self
            .pop_stack()
            .map_err(|e| format!("exec_pop failed: {}", e))?;
        // TODO: check boundaries
        self.get_segment_mut(segment)[index as usize] = value;
        Ok(())
    }
    fn exec_copy_seg(
        &mut self,
        from_segment: Segment,
        from_index: u16,
        to_segment: Segment,
        to_index: u16,
    ) -> Result<(), String> {
        let value: i32 = match from_segment {
            Segment::Constant => from_index as i32,
            _ => self.get_segment(from_segment)[from_index as usize],
        };
        self.get_segment_mut(to_segment)[to_index as usize] = value;
        Ok(())
    }
    fn exec_internal(&mut self, internal_id: usize, num_args: usize) -> Result<(), String> {
        match internal_id {
            0 => {
                // Math.divide
                if num_args != 2 {
                    panic!(
                        "Expected Math.divide to be called with 2 args, not {}",
                        num_args
                    );
                }
                let a = self
                    .pop_stack()
                    .map_err(|e| format!("exec_internal failed: {}", e))?;
                let b = self
                    .pop_stack()
                    .map_err(|e| format!("exec_internal failed: {}", e))?;
                self.push_stack(b / a);
            }
            1 => {
                // Math.multiply
                if num_args != 2 {
                    panic!(
                        "Expected Math.multiply to be called with 2 args, not {}",
                        num_args
                    );
                }
                let a = self
                    .pop_stack()
                    .map_err(|e| format!("exec_internal failed: {}", e))?;
                let b = self
                    .pop_stack()
                    .map_err(|e| format!("exec_internal failed: {}", e))?;
                self.push_stack(b * a);
            }
            _ => {
                panic!("Unknown internal function {}", internal_id);
            }
        };
        Ok(())
    }

    fn exec_call(&mut self, function_ref: InCodeFuncRef, num_args: usize) -> Result<(), String> {
        self.push_stack((self.frame().index + 1) as i32); // return address
        self.push_stack(self.ram[LCL]);
        self.push_stack(self.ram[ARG]);
        self.push_stack(self.ram[THIS]);
        self.push_stack(self.ram[THAT]);
        self.ram[ARG] = self.ram[SP] - 5 - num_args as i32;
        self.ram[LCL] = self.ram[SP];

        self.call_stack
            .push(VMStackFrame::new(function_ref, num_args));
        Ok(())
    }
    fn exec_return(&mut self) -> Result<(), String> {
        let return_value = self.pop_stack()?;

        let arg = self.ram[ARG];
        self.ram[SP] = self.ram[LCL];
        self.ram[THAT] = self.pop_global_stack()?;
        self.ram[THIS] = self.pop_global_stack()?;
        self.ram[ARG] = self.pop_global_stack()?;
        self.ram[LCL] = self.pop_global_stack()?;
        let _return_index = self.pop_global_stack()?;

        self.ram[SP] = arg;
        self.push_global_stack(return_value);

        self.call_stack.pop();
        self.frame_mut().index += 1;
        Ok(())
    }

    pub fn init(&mut self) -> Result<(), String> {
        self.ram[SP] = 256;
        if let Some(init_func) = self.program.get_function_ref("Sys.init") {
            self.call_stack.push(VMStackFrame::new(init_func, 0));
            return Ok(());
        }
        return Err("No Sys.init function found".to_string());
    }

    fn exec_arithmetic(&mut self, op: Operation) -> Result<(), String> {
        use Operation::*;
        let result = match op {
            Neg | Not => {
                let a = self.pop_stack()?;
                match op {
                    Neg => -a,
                    Not => !a,
                    _ => panic!("This should never happen"),
                }
            }
            _ => {
                let a = self.pop_stack()?;
                let b = self.pop_stack()?;
                match op {
                    Neg | Not => panic!("This should never happen"),
                    Add => b + a,
                    Sub => b - a,
                    And => b & a,
                    Or => b | a,
                    Eq => {
                        if b == a {
                            -1
                        } else {
                            0
                        }
                    }
                    Lt => {
                        if b < a {
                            -1
                        } else {
                            0
                        }
                    }
                    Gt => {
                        if b > a {
                            -1
                        } else {
                            0
                        }
                    }
                }
            }
        };
        self.push_stack(result);
        Ok(())
    }

    fn next_command(&self) -> Option<&Command> {
        if let Some(frame) = self.call_stack.last() {
            Some(self.program.get_command(&frame.function, frame.index))
        } else {
            None
        }
    }

    pub fn run(&mut self, max_steps: usize) -> Result<i32, String> {
        self.init()?;
        loop {
            if let Some(result) = self.step()? {
                return Ok(result);
            }
            if self.step_counter > max_steps {
                return Err(format!(
                    "Program failed to finish within {} steps",
                    max_steps
                ));
            }
        }
    }

    pub fn get_internals() -> HashMap<String, usize> {
        let mut internals: HashMap<String, usize> = HashMap::new();
        internals.insert("Math.divide".to_string(), 0);
        internals.insert("Math.multiply".to_string(), 1);
        internals
    }

    pub fn profile_step(&mut self) {
        self.profiler
            .count_function_step(FunctionRef::InCode(self.frame().function));
        if let Some(command) = self.next_command() {
            let command = *command;
            if let Command::Call(function_ref, _) = command {
                self.profiler.count_function_call(function_ref.clone());
            }
        }
    }

    pub fn step(&mut self) -> Result<Option<i32>, String> {
        self.step_counter += 1;
        let command = self
            .next_command()
            .ok_or("No more commands to execute".to_string())?;
        let command = *command;
        match command {
            // Function commands
            Command::Function(_, num_locals) => {
                for _ in 0..num_locals {
                    self.push_global_stack(0);
                }
                self.frame_mut().index += 1;
            }
            Command::Call(function_ref, num_args) => match function_ref {
                FunctionRef::Internal(internal_id) => {
                    self.exec_internal(internal_id, num_args as usize)
                        .map_err(|e| format!("failed step {:?}: {}", command, e))?;
                    self.frame_mut().index += 1;
                }
                FunctionRef::InCode(in_code_func_ref) => {
                    self.exec_call(in_code_func_ref, num_args as usize)
                        .map_err(|e| format!("failed step {:?}: {}", command, e))?;
                }
            },
            Command::Return => {
                if self.call_stack.len() == 1 {
                    // There is nowhere left to return to,
                    // so assume this means returning a value
                    // from the entire program to whatever system
                    // might be running it.
                    return Ok(Some(self.peek_stack()));
                }
                self.exec_return()?;
            }
            // goto commands
            Command::Goto(index) => {
                self.frame_mut().index = index;
            }
            Command::If(index) => {
                if self
                    .pop_stack()
                    .map_err(|e| format!("failed step {:?}: {} {}", command, e, self.debug()))?
                    == -1
                {
                    self.frame_mut().index = index;
                } else {
                    self.frame_mut().index += 1;
                }
            }
            // stack commands
            Command::Push(segment, index) => {
                self.exec_push(segment, index)
                    .map_err(|e| format!("failed step {:?}: {}", command, e))?;
                self.frame_mut().index += 1;
            }
            Command::Pop(segment, index) => {
                self.exec_pop(segment, index)
                    .map_err(|e| format!("failed step {:?}: {}", command, e))?;
                self.frame_mut().index += 1;
            }
            Command::CopySeg {
                from_segment,
                from_index,
                to_segment,
                to_index,
            } => {
                self.exec_copy_seg(from_segment, from_index, to_segment, to_index)
                    .map_err(|e| format!("failed step {:?}: {}", command, e))?;
                self.frame_mut().index += 1;
            }
            // arithmetic commands
            Command::Arithmetic(op) => {
                self.exec_arithmetic(op)
                    .map_err(|e| format!("failed step {:?}: {}", command, e))?;
                self.frame_mut().index += 1;
            }
        };
        return Ok(None);
    }

    fn debug(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        writeln!(&mut s, "Step: {}", self.step_counter).unwrap();
        writeln!(&mut s, "Call Stack:").unwrap();
        for frame in self.call_stack.iter() {
            let func_name = self
                .program
                .get_function_name(&frame.function.to_function_ref())
                .unwrap_or("Unknown Function");
            writeln!(&mut s, "  {}", func_name).unwrap();
        }
        writeln!(&mut s, "Stack: {:?}", self.get_stack()).unwrap();
        for segment in SEGMENTS.iter() {
            match segment {
                Segment::Static | Segment::Temp => {
                    writeln!(&mut s, "{:?}: {:?}", segment, self.get_segment(*segment)).unwrap();
                }
                _ => {}
            }
        }
        writeln!(&mut s, "Next Command: {:?}", self.next_command()).unwrap();
        return s;
    }

    pub fn profiler_stats(&self) -> String {
        let mut stats = self.profiler.function_stats.iter().collect::<Vec<_>>();
        stats.sort_by_key(|(_func_ref, stats)| stats.num_steps);
        let total_steps: u64 = stats.iter().map(|(_, stats)| stats.num_steps).sum();
        let top = format!(
            "{:<30} {:>10} {:>10} {:>10} {:>10}",
            "function", "calls", "steps", "steps/call", "% steps"
        );
        let body = stats
            .iter()
            .map(|(func_ref, stats)| {
                format!(
                    "{:.<30} {:>10} {:>10} {:>10} {:>10.2}%",
                    if let Some(name) = self.program.get_function_name(func_ref) {
                        name
                    } else {
                        "UNKNOWN_FUNC"
                    },
                    stats.num_calls,
                    stats.num_steps,
                    if stats.num_calls > 0 {
                        stats.num_steps / stats.num_calls
                    } else {
                        0
                    },
                    stats.num_steps as f64 / total_steps as f64 * 100.0
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n{}", top, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod vmemulator {
        use super::*;

        mod global_stack {
            use super::*;
            fn setup_vm() -> VMEmulator {
                let mut vm = VMEmulator::new(
                    VMProgram::new(&vec![(
                        "Sys.vm",
                        "
                    function Sys.init 0
                    return",
                    )])
                    .unwrap(),
                );
                vm.init().unwrap();
                return vm;
            }
            #[test]
            fn global_push() {
                let mut vm = setup_vm();
                assert_eq!(
                    vm.get_global_stack().len(),
                    0,
                    "global stack should initially be empty"
                );
                vm.push_global_stack(10);
                assert_eq!(
                    vm.get_global_stack(),
                    &[10],
                    "global stack should contain pushed value"
                );
                assert_eq!(vm.ram[256], 10, "global stack should start at ram[256]");
                vm.push_global_stack(20);
                vm.push_global_stack(30);
                assert_eq!(
                    vm.get_global_stack(),
                    &[10, 20, 30],
                    "global stack should contain pushed values"
                );
            }
            #[test]
            fn global_pop() {
                let mut vm = setup_vm();
                vm.ram[SP] = 258;
                vm.ram[256] = 34;
                vm.ram[257] = 120;
                assert_eq!(vm.get_global_stack(), &[34, 120]);
                assert_eq!(
                    vm.pop_global_stack(),
                    Ok(120),
                    "Popping should return the top value on the stack"
                );
                assert_eq!(
                    vm.get_global_stack(),
                    &[34],
                    "Popping should remove the top item on the stack"
                );
                assert_eq!(
                    vm.pop_global_stack(),
                    Ok(34),
                    "Popping should return the top value on the stack"
                );
                assert_eq!(vm.get_global_stack(), &[]);
                assert!(
                    vm.pop_global_stack().is_err(),
                    "Popping off an empty stack should return an error"
                );
            }
        }

        mod segments {
            use super::*;

            fn setup_vm() -> VMEmulator {
                let mut vm = VMEmulator::new(
                    VMProgram::new(&vec![
                        (
                            "Sys.vm",
                            "
                                function Sys.init 2
                                push constant 10
                                pop static 0
                                return",
                        ),
                        (
                            "Main.vm",
                            "
                                function Main.main 3
                                push constant 30
                                pop static 4
                                return",
                        ),
                    ])
                    .unwrap(),
                );
                vm.init().unwrap();
                return vm;
            }

            #[test]
            fn static_segment() {
                let mut vm = setup_vm();
                assert_eq!(
                    vm.get_segment(Segment::Static),
                    &[0],
                    "Static segment should be initialized to 0s"
                );
                vm.push_stack(10);
                assert_eq!(vm.get_stack(), &[10]);
                vm.exec_pop(Segment::Static, 0).unwrap();
                assert_eq!(
                    vm.get_segment(Segment::Static),
                    &[10],
                    "Static segment should be written to with value popped from stack"
                );
                assert_eq!(vm.get_stack(), &[], "pop should always pop off the stack");
                assert_eq!(vm.ram[16], 10, "static segment should be at ram[16]");

                // call into another function...
                vm.exec_call(vm.program.get_function_ref("Main.main").unwrap(), 0)
                    .unwrap();
                assert_eq!(vm.get_segment(Segment::Static), &[0, 0, 0, 0, 0]);
                vm.push_stack(25);
                vm.exec_pop(Segment::Static, 3).unwrap();
                assert_eq!(vm.get_segment(Segment::Static), &[0, 0, 0, 25, 0]);

                assert_eq!(
                    vm.ram[16..(16 + 1 + 5)],
                    [10, 0, 0, 0, 25, 0],
                    "static segments should be stored contiguously in ram"
                );
            }

            #[test]
            fn temp_segment() {
                let mut vm = setup_vm();
                vm.push_stack(10);
                vm.exec_pop(Segment::Temp, 0).unwrap();
                vm.push_stack(20);
                vm.exec_pop(Segment::Temp, 7).unwrap();
                assert_eq!(
                    vm.ram[5..5 + 8],
                    [10, 0, 0, 0, 0, 0, 0, 20],
                    "Temp segment should be in ram[5..13]"
                );
                assert_eq!(vm.get_segment(Segment::Temp), &[10, 0, 0, 0, 0, 0, 0, 20]);
            }

            #[test]
            fn pointer_segment() {
                let mut vm = setup_vm();
                assert_eq!(vm.get_segment(Segment::Pointer), &[0, 0]);
                vm.push_stack(10);
                vm.exec_pop(Segment::Pointer, 0).unwrap();
                vm.push_stack(20);
                vm.exec_pop(Segment::Pointer, 1).unwrap();
                assert_eq!(
                    vm.ram[3..5],
                    [10, 20],
                    "pointer segment should be in ram[3..5]"
                );
                assert_eq!(vm.get_segment(Segment::Pointer), &[10, 20]);
            }

            #[test]
            fn this_that_segment() {
                let mut vm = setup_vm();
                assert_eq!(vm.get_segment(Segment::Pointer), &[0, 0]);
                assert_eq!(vm.get_segment(Segment::This), &vm.ram);

                vm.ram[3000] = 10;
                vm.ram[4000] = 25;

                vm.get_segment_mut(Segment::Pointer)[0] = 3000;
                vm.get_segment_mut(Segment::Pointer)[1] = 4000;

                assert_eq!(vm.get_segment(Segment::This)[0], 10);
                assert_eq!(vm.get_segment(Segment::That)[0], 25);
            }

            #[test]
            fn local_segment() {
                let mut vm = setup_vm();
                vm.ram[1] = 270;
                assert_eq!(vm.get_segment(Segment::Local), &[0, 0]);
                vm.push_stack(5);
                vm.exec_pop(Segment::Local, 0).unwrap();
                vm.push_stack(25);
                vm.exec_pop(Segment::Local, 1).unwrap();
                assert_eq!(vm.get_segment(Segment::Local), &[5, 25]);
                assert_eq!(
                    vm.ram[270..272],
                    [5, 25],
                    "Local segment should be located wherever ram[1] points to"
                );
            }

            #[test]
            fn argument_segment() {
                let mut vm = setup_vm();
                vm.exec_call(vm.program.get_function_ref("Main.main").unwrap(), 2)
                    .unwrap();
                vm.ram[2] = 270;
                vm.ram[270] = 13;
                vm.ram[271] = 14;
                assert_eq!(
                    vm.get_segment(Segment::Argument),
                    &[13, 14],
                    "Argument segment should be located wherever ram[2] points to"
                );
            }
        }

        mod functions {
            use super::*;

            fn setup_vm() -> VMEmulator {
                let mut vm = VMEmulator::new(
                    VMProgram::new(&vec![
                        (
                            "Sys.vm",
                            "
                                function Sys.init 0
                                push constant 10
                                pop constant 20
                                call Main.add 2
                                return",
                        ),
                        (
                            "Main.vm",
                            "
                                function Main.add 0
                                push argument 0
                                push argument 1
                                add
                                return",
                        ),
                    ])
                    .unwrap(),
                );
                vm.init().unwrap();
                return vm;
            }

            #[test]
            fn function_calls() {
                let mut vm = setup_vm();
                vm.push_stack(2);
                vm.push_stack(3);
                vm.exec_call(vm.program.get_function_ref("Main.add").unwrap(), 2)
                    .unwrap();
                assert_eq!(
                    vm.get_segment(Segment::Argument),
                    &[2, 3],
                    "After a call, argument segment should point to arguments from stack"
                );
                assert_eq!(vm.ram[256..258], [2, 3]);
            }
        }
    }

    #[test]
    fn test_vmemulator_run() {
        let program = VMProgram::new(&vec![(
            "Sys.vm",
            "
            function Sys.init 0
                push constant 10
            return
            ",
        )])
        .unwrap();

        let mut vm = VMEmulator::new(program);
        let result = vm.run(1000).expect("failed to run program");
        assert_eq!(
            result, 10,
            "Expected program to end by returning 10. Got {}",
            result
        );
    }

    #[test]
    fn test_math_divide() {
        let program = VMProgram::with_internals(
            &vec![(
                "Sys.vm",
                "
                function Sys.init 0
                    push constant 10
                    push constant 2
                    call Math.divide 2
                return
                ",
            )],
            Some(VMEmulator::get_internals()),
        )
        .unwrap();
        let mut vm = VMEmulator::new(program);
        let result = vm.run(1000).expect("failed to run program");
        assert_eq!(
            result, 5,
            "Expected program to end by returning 5. Got {}",
            result
        );
    }

    #[test]
    fn test_vmemulator_init() {
        let program = VMProgram::new(&vec![(
            "Sys.vm",
            "
            function Sys.init 0
                push constant 10
                pop static 0
                label LOOP
                call Sys.incr 0
                pop temp 0
                goto LOOP
            return
            
            function Sys.incr 0
                push static 0
                push constant 1
                add
                pop static 0
                push static 0
            return
            ",
        )])
        .unwrap();

        let mut vm = VMEmulator::new(program);
        assert_eq!(vm.init(), Ok(()));

        // function Sys.init 0
        vm.step().unwrap();
        println!("step1: {}", vm.debug());

        // push constant 10
        // pop static 0
        vm.step().unwrap();
        println!("step2: {}", vm.debug());
        assert_eq!(vm.get_stack().len(), 0);
        assert_eq!(vm.get_segment(Segment::Static)[0], 10);

        // call Sys.incr 0
        vm.step().unwrap();
        println!("{}", vm.debug());

        // function Sys.incr 0
        vm.step().unwrap();
        println!("{}", vm.debug());

        // push static 0
        vm.step().unwrap();
        println!("{}", vm.debug());
        // push constant 1
        vm.step().unwrap();
        println!("{}", vm.debug());

        // add
        vm.step().unwrap();
        println!("{}", vm.debug());

        // pop static 0
        vm.step().unwrap();
        println!("{}", vm.debug());
        // push static 0
        vm.step().unwrap();
        println!("{}", vm.debug());
        // return
        vm.step().unwrap();
        println!("{}", vm.debug());

        for _ in 0..100 {
            vm.step().unwrap();
            println!("{}", vm.debug());
        }
    }
}
