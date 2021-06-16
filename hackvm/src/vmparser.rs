use super::vmcommand::Segment;
use std::fmt;

#[derive(PartialEq, Clone, Debug)]
pub enum Token {
    None,

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

    // stack commands
    Push(Segment, u16),
    Pop(Segment, u16),

    // goto commands
    Label(String),
    If(String),
    Goto(String),

    // function commands
    Function(String, u16),
    Return,
    Call(String, u16),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Token::None => "<none>".to_string(),
            // arithmetic commands
            Token::Neg
            | Token::Not
            | Token::Add
            | Token::Sub
            | Token::And
            | Token::Or
            | Token::Eq
            | Token::Lt
            | Token::Gt => format!("{:?}", self).to_lowercase(),

            // stack commands
            Token::Push(segment, index) => format!("push {} {}", segment, index),
            Token::Pop(segment, index) => format!("pop {} {}", segment, index),

            // goto commands
            Token::Label(label) => format!("label {}", label),
            Token::If(label) => format!("if-goto {}", label),
            Token::Goto(label) => format!("goto {}", label),

            // function commands
            Token::Function(func_name, num_locals) => {
                format!("function {} {}", func_name, num_locals)
            }
            Token::Return => "return".to_string(),
            Token::Call(func_name, num_args) => format!("call {} {}", func_name, num_args),
        };
        f.write_str(&s)
    }
}

fn parse_segment(s: &str) -> Result<Segment, String> {
    match s {
        "constant" => Ok(Segment::Constant),
        "argument" => Ok(Segment::Argument),
        "local" => Ok(Segment::Local),
        "static" => Ok(Segment::Static),
        "this" => Ok(Segment::This),
        "that" => Ok(Segment::That),
        "pointer" => Ok(Segment::Pointer),
        "temp" => Ok(Segment::Temp),
        _ => Err(format!("Invalid segment {:?}", s)),
    }
}

fn parse_line(line: &str) -> Result<Token, String> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    match parts.get(0) {
        None => Ok(Token::None),
        Some(command) => match *command {
            // comment
            "//" => Ok(Token::None),

            // arithmetic commands
            "neg" => Ok(Token::Neg),
            "not" => Ok(Token::Not),
            "add" => Ok(Token::Add),
            "sub" => Ok(Token::Sub),
            "and" => Ok(Token::And),
            "or" => Ok(Token::Or),
            "eq" => Ok(Token::Eq),
            "lt" => Ok(Token::Lt),
            "gt" => Ok(Token::Gt),

            // goto commands
            "label" | "if-goto" | "goto" => {
                let arg1 = parts
                    .get(1)
                    .ok_or(format!("Missing label in {:?} command", command))?
                    .to_string();
                Ok(match *command {
                    "label" => Token::Label(arg1),
                    "if-goto" => Token::If(arg1),
                    "goto" => Token::Goto(arg1),
                    _ => panic!("This should never happen. Command: {}", command),
                })
            }
            // stack commands
            "push" | "pop" => {
                let arg1 = parts
                    .get(1)
                    .ok_or(format!("Missing segment in {:?} command", command))?;
                let arg2 = parts
                    .get(2)
                    .ok_or(format!("Missing index in {:?} command", command))?;
                let segment = parse_segment(arg1)?;
                let index = arg2
                    .parse::<u16>()
                    .map_err(|_| format!("Invalid index {:?} in {:?} command", arg2, command))?;
                Ok(match *command {
                    "push" => Token::Push(segment, index),
                    "pop" => Token::Pop(segment, index),
                    _ => panic!("This should never happen"),
                })
            }

            // function calling commands
            "return" => Ok(Token::Return),
            "function" | "call" => {
                let arg1 = parts
                    .get(1)
                    .ok_or(format!("Missing function name in {:?} command", command))?
                    .to_string();
                let arg2 = parts
                    .get(2)
                    .ok_or(format!("Missing arg2 in {:?} command", command))?
                    .to_string();
                let num = arg2.parse::<u16>().map_err(|_| "Invalid num".to_string())?;
                Ok(match *command {
                    "function" => Token::Function(arg1, num),
                    "call" => Token::Call(arg1, num),
                    _ => panic!("This should never happen"),
                })
            }

            _ => Err(format!("Could not parse line {}", line)),
        },
    }
}

pub fn parse_lines(lines: &str) -> Result<Vec<Token>, String> {
    let mut tokens: Vec<Token> = Vec::new();
    for line in lines.lines() {
        let token = parse_line(line)?;
        if token != Token::None {
            tokens.push(token);
        }
    }
    return Ok(tokens);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_line() {
        assert_eq!(parse_line("add"), Ok(Token::Add));
        assert_eq!(
            parse_line("push constant 10"),
            Ok(Token::Push(Segment::Constant, 10))
        );
        assert_eq!(
            parse_line("push foo 10"),
            Err("Invalid segment \"foo\"".to_string())
        )
    }

    #[test]
    fn test_parse_lines() {
        assert_eq!(
            parse_lines(
                "
// a simple function
function foo 2

    push constant 3
    not

return // the end"
            ),
            Ok(vec![
                Token::Function("foo".to_string(), 2),
                Token::Push(Segment::Constant, 3),
                Token::Not,
                Token::Return,
            ])
        );
    }
}
