use super::vmparser::{parse_lines, Token};
use std::cmp;
use std::collections::HashMap;
use std::fmt;

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Segment {
    Constant,
    Argument,
    Local,
    Static,
    This,
    That,
    Pointer,
    Temp,
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format!("{:?}", self).to_lowercase())
    }
}

#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug)]
pub struct InCodeFuncRef {
    file_index: usize,
    function_index: usize,
}

impl InCodeFuncRef {
    pub fn to_function_ref(self) -> FunctionRef {
        FunctionRef::InCode(self)
    }
}

#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug)]
pub enum FunctionRef {
    Internal(usize),
    InCode(InCodeFuncRef),
}

impl FunctionRef {
    pub fn new(file_index: usize, function_index: usize) -> FunctionRef {
        FunctionRef::InCode(InCodeFuncRef {
            file_index,
            function_index,
        })
    }
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Operation {
    // arithmetic commands
    Neg,
    Not,
    Add,
    Sub,
    And,
    Or,
    Eq,
    Lt,
    Gt,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum Command {
    Arithmetic(Operation),

    // stack commands
    Push(Segment, u16),
    Pop(Segment, u16),

    // goto commands
    If(usize),
    Goto(usize),

    // function commands
    Function(FunctionRef, u16),
    Return,
    Call(FunctionRef, u16),

    // optimized commands
    CopySeg {
        from_segment: Segment,
        from_index: u16,
        to_segment: Segment,
        to_index: u16,
    },
}

impl Command {
    pub fn to_string(&self, program: &VMProgram) -> String {
        match self {
            Command::Arithmetic(op) => format!("{:?}", op).to_lowercase(),
            Command::Push(segment, index) => {
                format!("push {} {}", segment, index)
            }
            Command::Pop(segment, index) => {
                format!("pop {} {}", segment, index)
            }
            Command::If(index) => {
                format!("if-goto {}", index)
            }
            Command::Goto(index) => {
                format!("goto {}", index)
            }
            Command::Function(func_ref, num_locals) => {
                format!(
                    "function {} {}",
                    program
                        .get_function_name(func_ref)
                        .unwrap_or("Unknown Function"),
                    num_locals
                )
            }
            Command::Return => "return".to_string(),
            Command::Call(func_ref, num_args) => {
                format!(
                    "call {} {}",
                    program
                        .get_function_name(func_ref)
                        .unwrap_or("Unknown Function"),
                    num_args
                )
            }
            Command::CopySeg {
                from_segment,
                from_index,
                to_segment,
                to_index,
            } => {
                format!(
                    "{}\n{}",
                    Command::Push(*from_segment, *from_index).to_string(program),
                    Command::Push(*to_segment, *to_index).to_string(program)
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct VMFunction {
    pub id: FunctionRef,
    pub name: String,
    pub num_locals: usize,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone)]
pub struct VMFile {
    pub name: String,
    pub functions: Vec<VMFunction>,
    pub num_statics: usize,
    pub static_offset: usize,
}

type FunctionTable = bimap::BiMap<String, FunctionRef>;
type LabelTable = HashMap<String, usize>;

#[derive(PartialEq, Clone, Debug)]
enum OptimizedToken {
    Base(Token),
    CopySeg {
        from_segment: Segment,
        from_index: u16,
        to_segment: Segment,
        to_index: u16,
    },
}

struct TokenizedFunction {
    name: String,
    commands: Vec<Token>,
}
impl TokenizedFunction {
    fn from_tokens(tokens: &[Token]) -> Result<TokenizedFunction, String> {
        let first_token = &tokens
            .get(0)
            .ok_or_else(|| "Failed creating tokenized functions. No tokens provided.")?;
        if let Token::Function(func_name, _) = first_token {
            Ok(TokenizedFunction {
                name: func_name.clone(),
                commands: tokens.to_vec(),
            })
        } else {
            Err(format!("Failed creating tokenized function. Tokens don't start with a function declaration. Found {:?} instead.", first_token))
        }
    }
    fn get_optimized_tokens(&self) -> Vec<OptimizedToken> {
        let mut optimized: Vec<OptimizedToken> = Vec::new();
        let mut i = 0;
        while i < self.commands.len() {
            if i + 1 < self.commands.len() {
                let a = &self.commands[i];
                let b = &self.commands[i + 1];
                match (a, b) {
                    (Token::Push(from_segment, from_index), Token::Pop(to_segment, to_index)) => {
                        optimized.push(OptimizedToken::CopySeg {
                            from_segment: *from_segment,
                            from_index: *from_index,
                            to_segment: *to_segment,
                            to_index: *to_index,
                        });
                        i += 2;
                    }
                    _ => {
                        optimized.push(OptimizedToken::Base(a.clone()));
                        i += 1;
                    }
                }
            } else {
                optimized.push(OptimizedToken::Base(self.commands[i].clone()));
                i += 1;
            }
        }
        return optimized;
    }
}
struct TokenizedFunctionOptimized {
    name: String,
    label_table: LabelTable,
    commands: Vec<OptimizedToken>,
}

impl TokenizedFunctionOptimized {
    fn from(tokenized_func: TokenizedFunction) -> Result<TokenizedFunctionOptimized, String> {
        let optimized = tokenized_func.get_optimized_tokens();
        let (label_table, command_tokens) =
            TokenizedFunctionOptimized::build_label_table(&optimized).map_err(|e| {
                format!(
                    "failed building label table for {}: {}",
                    tokenized_func.name, e
                )
            })?;
        Ok(TokenizedFunctionOptimized {
            name: tokenized_func.name,
            label_table,
            commands: command_tokens,
        })
    }

    fn from_tokens(tokens: &[Token]) -> Result<TokenizedFunctionOptimized, String> {
        let tokenized_func = TokenizedFunction::from_tokens(tokens)?;
        Self::from(tokenized_func)
    }

    fn build_label_table(
        func_tokens: &[OptimizedToken],
    ) -> Result<(LabelTable, Vec<OptimizedToken>), String> {
        let mut label_table: LabelTable = HashMap::new();
        let mut command_index = 0;
        let mut command_tokens: Vec<OptimizedToken> = Vec::new();
        for token in func_tokens.iter() {
            if let OptimizedToken::Base(Token::Label(label)) = token {
                match label_table.get(label) {
                    Some(_) => return Err(format!("label {:?} declared twice", label)),
                    None => {
                        println!("Inserting label {}", label);
                        label_table.insert(label.to_string(), command_index);
                    }
                }
            } else {
                command_tokens.push(token.clone());
                command_index += 1;
            }
        }
        return Ok((label_table, command_tokens));
    }
}

struct TokenizedFile {
    name: String,
    functions: Vec<TokenizedFunction>,
}

impl TokenizedFile {
    fn from_tokens(name: &str, tokens: &[Token]) -> Result<TokenizedFile, String> {
        let iter = GroupByBound::new(tokens, |token| match token {
            Token::Function(_, _) => true,
            _ => false,
        });
        let funcs = iter
            .map(TokenizedFunction::from_tokens)
            .collect::<Result<Vec<_>, String>>()
            .map_err(|e| format!("Failed tokenizing file: {}", e))?;
        return Ok(TokenizedFile {
            name: name.to_string(),
            functions: funcs,
        });
    }
}

struct TokenizedProgram {
    files: Vec<TokenizedFile>,
}

impl TokenizedProgram {
    fn from_files(files: &[(&str, &str)]) -> Result<TokenizedProgram, String> {
        let tokenized_files = files
            .iter()
            .map(|(filename, content)| {
                parse_lines(content)
                    .map(|tokens| (filename, tokens))
                    .map_err(|e| {
                        format!(
                            "Failed tokenizing program: Couldn't parse {}: {}",
                            filename, e
                        )
                    })
            })
            .map(|result| {
                let (filename, file_tokens) = result?;
                if file_tokens.len() == 0 {
                    Err(format!(
                        "Failed tokenizing program: File {} has no vm commands",
                        filename
                    ))
                } else {
                    TokenizedFile::from_tokens(*filename, &file_tokens)
                }
            })
            .collect::<Result<Vec<TokenizedFile>, String>>()?;
        Ok(TokenizedProgram {
            files: tokenized_files,
        })
    }
}

#[derive(Clone)]
pub struct VMProgram {
    pub files: Vec<VMFile>,
    pub function_table: FunctionTable,
    pub warnings: Vec<Box<str>>,
}

impl VMProgram {
    pub fn get_function_name(&self, func_ref: &FunctionRef) -> Option<&str> {
        self.function_table.get_by_right(func_ref).map(|s| &s[..])
    }
    pub fn get_function_ref(&self, name: &str) -> Option<InCodeFuncRef> {
        if let Some(FunctionRef::InCode(in_code_ref)) =
            self.function_table.get_by_left(&name.to_string())
        {
            Some(*in_code_ref)
        } else {
            None
        }
    }

    pub fn get_vmfunction(&self, func_ref: &InCodeFuncRef) -> &VMFunction {
        &self.files[func_ref.file_index].functions[func_ref.function_index]
    }

    pub fn get_command(&self, func_ref: &InCodeFuncRef, index: usize) -> &Command {
        &self.get_vmfunction(func_ref).commands[index]
    }

    pub fn get_file(&self, func_ref: &InCodeFuncRef) -> &VMFile {
        &self.files[func_ref.file_index]
    }

    pub fn empty() -> VMProgram {
        VMProgram {
            files: Vec::new(),
            function_table: bimap::BiMap::new(),
            warnings: Vec::new(),
        }
    }

    pub fn new(files: &Vec<(&str, &str)>) -> Result<VMProgram, String> {
        VMProgram::with_internals(files, None)
    }
    pub fn with_internals(
        files: &Vec<(&str, &str)>,
        internal_funcs: Option<HashMap<&'static str, FunctionRef>>,
    ) -> Result<VMProgram, String> {
        let tokenized_program = TokenizedProgram::from_files(files)
            .map_err(|e| format!("Failed to create VMProgram: {}", e))?;

        let mut function_table: FunctionTable = bimap::BiMap::new();
        // let mut tokenized_files: Vec<TokenizedFile> = Vec::new();
        let mut warnings: Vec<Box<str>> = Vec::new();
        // tokenize files and build function table
        if let Some(internal_funcs) = internal_funcs {
            for (func_name, internal_func_ref) in internal_funcs.iter() {
                function_table.insert(func_name.to_string(), *internal_func_ref);
            }
        }

        for (file_index, tokenized_file) in tokenized_program.files.iter().enumerate() {
            for (function_index, tokenized_func) in tokenized_file.functions.iter().enumerate() {
                match function_table.get_by_left(&tokenized_func.name) {
                    Some(FunctionRef::InCode { .. }) => {
                        return Err(format!("function {:?} declared twice", tokenized_func.name));
                    }
                    Some(FunctionRef::Internal(_)) => {
                        // We ignore implementations of internal functions that appear in code.
                    }
                    None => {
                        function_table.insert(
                            tokenized_func.name.clone(),
                            FunctionRef::new(file_index, function_index),
                        );
                    }
                }
            }
        }

        let mut files: Vec<VMFile> = Vec::new();
        // process files into vm commands
        let mut static_offset = 0_usize;
        for tokenized_file in tokenized_program.files.into_iter() {
            let mut vmfile = VMFile {
                name: tokenized_file.name.clone(),
                functions: Vec::new(),
                num_statics: 0,
                static_offset,
            };
            for tokenized_func in tokenized_file
                .functions
                .into_iter()
                .map(TokenizedFunctionOptimized::from)
            {
                let tokenized_func = tokenized_func?;
                let label_table = &tokenized_func.label_table;
                let mut tokens = tokenized_func.commands.iter();
                if let OptimizedToken::Base(Token::Function(func_name, num_locals)) =
                    tokens.next().unwrap()
                {
                    let function_ref = *function_table
                        .get_by_left(&func_name.to_string())
                        .expect("Expected to find function name in function table");
                    let mut vmfunc = VMFunction {
                        id: function_ref,
                        name: func_name.to_string(),
                        num_locals: *num_locals as usize,
                        commands: vec![Command::Function(function_ref, *num_locals)],
                    };

                    for token in tokens {
                        let command = match token {
                            OptimizedToken::CopySeg {
                                from_segment,
                                from_index,
                                to_segment,
                                to_index,
                            } => {
                                if *from_segment == Segment::Static {
                                    vmfile.num_statics =
                                        cmp::max(vmfile.num_statics, (from_index + 1).into());
                                }
                                if *to_segment == Segment::Static {
                                    vmfile.num_statics =
                                        cmp::max(vmfile.num_statics, (to_index + 1).into());
                                }
                                Command::CopySeg {
                                    from_segment: *from_segment,
                                    from_index: *from_index,
                                    to_segment: *to_segment,
                                    to_index: *to_index,
                                }
                            }
                            OptimizedToken::Base(token) => match token {
                                // empty token... should have been filtered out earlier
                                Token::None => panic!("Didn't expect Token::None"),

                                // arithmetic commands
                                Token::Neg => Command::Arithmetic(Operation::Neg),
                                Token::Not => Command::Arithmetic(Operation::Not),
                                Token::Add => Command::Arithmetic(Operation::Add),
                                Token::Sub => Command::Arithmetic(Operation::Sub),
                                Token::And => Command::Arithmetic(Operation::And),
                                Token::Or => Command::Arithmetic(Operation::Or),
                                Token::Eq => Command::Arithmetic(Operation::Eq),
                                Token::Lt => Command::Arithmetic(Operation::Lt),
                                Token::Gt => Command::Arithmetic(Operation::Gt),

                                // function commands
                                Token::Function(_, _) => panic!("Didn't expect Token::Function"),
                                Token::Call(func_to_call, num_args) => {
                                    match function_table
                                        .get_by_left(&func_to_call.to_string())
                                        .copied()
                                    {
                                        Some(func_ref) => Command::Call(func_ref, *num_args),
                                        None => {
                                            warnings.push(
                                                format!(
                                                    "function {:?} does not exist",
                                                    func_to_call
                                                )
                                                .into_boxed_str(),
                                            );
                                            Command::Call(FunctionRef::new(1000, 1), *num_args)
                                        }
                                    }
                                }
                                Token::Return => Command::Return,

                                // goto commands
                                Token::Label(_) => panic!("Didn't expect Token::Label"),
                                Token::If(label) => {
                                    let index = label_table
                                        .get(label)
                                        .ok_or(format!("label {:?} does not exist", label))?;
                                    Command::If(*index)
                                }
                                Token::Goto(label) => {
                                    let index = label_table
                                        .get(label)
                                        .ok_or(format!("label {:?} does not exist", label))?;
                                    Command::Goto(*index)
                                }

                                // stack commands
                                // TODO: verify indexes for segments
                                Token::Push(segment, index) => {
                                    if *segment == Segment::Static {
                                        vmfile.num_statics =
                                            cmp::max(vmfile.num_statics, (index + 1).into());
                                    }
                                    Command::Push(*segment, *index)
                                }
                                Token::Pop(segment, index) => {
                                    if *segment == Segment::Static {
                                        vmfile.num_statics =
                                            cmp::max(vmfile.num_statics, (index + 1).into());
                                    }
                                    Command::Pop(*segment, *index)
                                }
                            },
                        };
                        vmfunc.commands.push(command);
                    }
                    vmfile.functions.push(vmfunc);
                } else {
                    panic!("Expected func to start with Token::Function");
                }
            }
            static_offset += vmfile.num_statics;
            files.push(vmfile);
        }
        return Ok(VMProgram {
            files,
            function_table,
            warnings,
        });
    }
}

struct GroupByBound<'a, T, P>
where
    P: Fn(&T) -> bool,
{
    data: &'a [T],
    pred: P,
    i: usize,
}

impl<'a, T, P> GroupByBound<'a, T, P>
where
    P: Fn(&T) -> bool,
{
    fn new(data: &'a [T], pred: P) -> GroupByBound<T, P> {
        GroupByBound { data, pred, i: 0 }
    }
}

impl<'a, T, P> Iterator for GroupByBound<'a, T, P>
where
    P: Fn(&T) -> bool,
{
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.data.len() {
            return None;
        }
        let mut j = self.i + 1;
        while j < self.data.len() && !(self.pred)(&self.data[j]) {
            j += 1;
        }
        let slice = &self.data[self.i..j];
        self.i = j;
        if slice.len() > 0 {
            return Some(slice);
        }
        return None;
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_group_by_bounds() {
        let v = vec![1, 2, 2, 0, 3, 0, 1, 65, 4, 3, 0, 1, 5, 4, 3, 0];
        let mut groups = GroupByBound::new(&v, |&t| t == 1);
        assert_eq!(groups.next(), Some(&v[0..6]));
        assert_eq!(groups.next(), Some(&v[6..11]));
        assert_eq!(groups.next(), Some(&v[11..]));
        assert_eq!(groups.next(), None);
    }

    #[test]
    fn test_with_internals() {
        let internals: HashMap<&'static str, FunctionRef> = HashMap::new();
        let files = vec![(
            "Sys.vm",
            "
            function Sys.init 1
                push constant 10
                push constant 11
                call Sys.add 2
            return",
        )];
        let program = VMProgram::with_internals(&files, Some(internals)).unwrap();
        assert_eq!(program.warnings.len(), 1, "Expected there to be warnings");
        assert_eq!(
            program.warnings[0].clone().into_string(),
            "function \"Sys.add\" does not exist"
        );
        assert_eq!(
            program.files[0].functions[0].commands[3],
            Command::Call(FunctionRef::new(1000, 1), 2),
            "Expected call to missing function to use another function ref..."
        );

        let mut internals: HashMap<&'static str, FunctionRef> = HashMap::new();
        internals.insert("Sys.add", FunctionRef::Internal(0));
        let program = VMProgram::with_internals(&files, Some(internals)).unwrap();
        assert_eq!(
            program.warnings.len(),
            0,
            "Expected there to be no warnings for calls to internal functions"
        );
        assert_eq!(
            program.files[0].functions[0].commands[3],
            Command::Call(FunctionRef::Internal(0), 2),
            "Expected call to internal function to use an internal function ref"
        );

        let files = vec![(
            "Sys.vm",
            "
            function Sys.init 1
                push constant 10
                push constant 11
                call Sys.add 2
            return

            function Sys.add 0
            push argument 0
            push argument 1
            add
            return
            ",
        )];
        let mut internals: HashMap<&'static str, FunctionRef> = HashMap::new();
        internals.insert("Sys.add", FunctionRef::Internal(0));
        let program = VMProgram::with_internals(&files, Some(internals)).unwrap();
        assert_eq!(
            program.warnings.len(),
            0,
            "Expected there to be no warnings for calls to internal functions"
        );
        assert_eq!(
            program.files[0].functions[0].commands[3],
            Command::Call(FunctionRef::Internal(0), 2),
            "Expected internal functions to take priority over functions of the same name defined in code"
        );
    }

    #[test]
    fn test_vmprogram_new() {
        let program = VMProgram::new(&vec![(
            "Sys.vm",
            "
            function Sys.init 1
                push constant 10
                pop static 0
                label LOOP
                call Sys.incr 0
                pop temp 0
                goto LOOP
            return
            
            function Sys.incr 1
                push static 0
                push constant 1
                add
                pop local 0
                push local 0
                push constant 10
                gt
                if-goto SAVE
                return
                label SAVE
                label SAVE2
                push local 0
                pop static 0
            return
            ",
        )])
        .unwrap();

        assert_eq!(program.files.len(), 1);
        assert_eq!(program.files[0].functions.len(), 2);
        assert_eq!(
            program.function_table.get_by_left("Sys.init").unwrap(),
            &FunctionRef::new(0, 0)
        );
        assert_eq!(
            program.function_table.get_by_left("Sys.incr").unwrap(),
            &FunctionRef::new(0, 1)
        );
        assert_eq!(
            program.files[0].functions[0].commands[0],
            Command::Function(FunctionRef::new(0, 0), 1),
        );
        assert_eq!(
            program.files[0].functions[0].commands[2],
            Command::Call(FunctionRef::new(0, 1), 0),
        );
    }
}
