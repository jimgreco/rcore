use std::fmt::{Debug, Display, Formatter};
use crate::command::lexer::{Command, Lexer, LexerContext, LexerError};

pub trait ExecutableCommandSpec {
    fn validate(&self, command: &Command) -> Result<bool, ParserError>;
    fn build(&self, command: &mut Command) -> Box<dyn ExecutableCommand>;
}

pub trait ExecutableCommand: Debug {
    fn execute(&self, context: &mut dyn LexerContext);
}

#[derive(Default)]
pub struct AssignmentCommandSpec {}

#[derive(Debug, PartialEq)]
pub struct AssignmentCommand {
    variable: String,
    value: String
}

#[derive(Default)]
pub struct DefaultAssignmentCommandSpec {}

#[derive(Debug, PartialEq)]
pub struct DefaultAssignmentCommand {
    variable: String,
    value: String
}

impl ExecutableCommandSpec for AssignmentCommandSpec {
    fn validate(&self, command: &Command) -> Result<bool, ParserError> {
        validate_assigment(command, "=")
    }

    fn build(&self, command: &mut Command) -> Box<dyn ExecutableCommand> {
        Box::new(AssignmentCommand {
            value: command.tokens.remove(2),
            variable: command.tokens.remove(0),
        })
    }
}

impl ExecutableCommand for AssignmentCommand {
    fn execute(&self, context: &mut dyn LexerContext) {
        context.set_value(&self.variable, &self.value, true);
    }
}

impl Display for AssignmentCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.variable)?;
        f.write_str(" = ")?;
        f.write_str(&self.value)
    }
}

impl ExecutableCommandSpec for DefaultAssignmentCommandSpec {
    fn validate(&self, command: &Command) -> Result<bool, ParserError> {
        validate_assigment(command, ":=")
    }

    fn build(&self, command: &mut Command) -> Box<dyn ExecutableCommand> {
        Box::new(DefaultAssignmentCommand {
            value: command.tokens.remove(2),
            variable: command.tokens.remove(0),
        })
    }
}

impl ExecutableCommand for DefaultAssignmentCommand {
    fn execute(&self, context: &mut dyn LexerContext) {
        context.set_value(&self.variable, &self.value, false);
    }
}

impl Display for DefaultAssignmentCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.variable)?;
        f.write_str(" := ")?;
        f.write_str(&self.value)
    }
}

fn validate_variable(variable: &str) -> bool {
    let mut first = true;

    for c in variable.chars() {
        if first && !c.is_alphabetic() && c != '_' {
            return false;
        } else if !c.is_alphanumeric() && c != '_' {
            return false;
        }
        first = false;
    }

    return true;
}

fn validate_assigment(command: &Command, sign: &'static str)
                      -> Result<bool, ParserError> {
    let tokens = &command.tokens;
    if tokens.len() == 3 && tokens[1] == sign {
        if validate_variable(&tokens[0]) {
            Ok(true)
        } else {
            Err(ParserError::InvalidVariableName {
                command: command.to_owned(),
                variable: tokens[0].to_owned(),
            })
        }
    } else {
        Ok(false)
    }
}

#[derive(Debug, PartialEq)]
pub enum ParserError {
    LexerError(LexerError),
    InvalidVariableName {
        command: Command,
        variable: String
    },
    InvalidCommand(Command)
}

pub struct Parser<'a> {
    file: &'a str,
    context: &'a dyn LexerContext,
    lexer: Lexer<'a>,
    command_specs: Vec<Box<dyn ExecutableCommandSpec>>,
    error: bool
}

impl<'a> Parser<'a> {
    pub fn new(file: &'a str, context: &'a dyn LexerContext) -> Parser<'a> {
        let mut parser = Parser {
            file,
            context,
            lexer: Lexer::new(file, context),
            command_specs: Vec::new(),
            error: false
        };
        parser.add_command(Box::new(AssignmentCommandSpec::default()));
        parser.add_command(Box::new(DefaultAssignmentCommandSpec::default()));
        parser
    }

    pub fn add_command(&mut self, spec: Box<dyn ExecutableCommandSpec>) {
        self.command_specs.push(spec);
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Box<dyn ExecutableCommand>, ParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.error {
            return None
        }

        match self.lexer.next() {
            Some(result) => {
                match result {
                    Ok(mut command) => {
                        for spec in &self.command_specs {
                            match spec.validate(&command) {
                                Ok(result) => if result {
                                    return Some(Ok(spec.build(&mut command)));
                                }
                                Err(e) => {
                                    self.error = true;
                                    return Some(Err(e))
                                }
                            }
                        }

                        self.error = true;
                        return Some(Err(ParserError::InvalidCommand(command)))
                    }
                    Err(e) => {
                        self.error = true;
                        Some(Err(ParserError::LexerError(e)))
                    }
                }
            },
            None => None
        }
    }
}

#[cfg(test)]
mod assignment_tests {
    use crate::command::lexer::Command;
    use crate::command::parser::{AssignmentCommandSpec, ExecutableCommandSpec, ParserError};

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = AssignmentCommandSpec::default();

        let result = spec.validate(&Command {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = AssignmentCommandSpec::default();

        let result = spec.validate(&Command {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = AssignmentCommandSpec::default();
        let command = Command {
            line: 0,
            tokens: vec!["12foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ParserError::InvalidVariableName { command, variable: "12foo".to_owned() }, result);
    }
}

#[cfg(test)]
mod default_assignment_tests {
    use crate::command::lexer::Command;
    use crate::command::parser::{DefaultAssignmentCommandSpec, ExecutableCommandSpec, ParserError};

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = DefaultAssignmentCommandSpec::default();

        let result = spec.validate(&Command {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = DefaultAssignmentCommandSpec::default();

        let result = spec.validate(&Command {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = DefaultAssignmentCommandSpec::default();
        let command = Command {
            line: 0,
            tokens: vec!["12foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ParserError::InvalidVariableName { command, variable: "12foo".to_owned() }, result);
    }
}

#[cfg(test)]
mod parser_tests {
    use crate::command::lexer::{Command, SimpleContext, LexerError, LexerContext};
    use crate::command::parser::{AssignmentCommand, DefaultAssignmentCommand, ExecutableCommand, ExecutableCommandSpec, Parser, ParserError, validate_variable};

    #[test]
    fn command_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            foo := soo
            do12 = goo
            ", &context);

        assert_eq!(format!("{:?}", parser.next().unwrap().unwrap()), format!("{:?}", AssignmentCommand {
            variable: "foo".to_owned(),
            value: "bar".to_owned(),
        }));
        assert_eq!(format!("{:?}", parser.next().unwrap().unwrap()), format!("{:?}", DefaultAssignmentCommand {
            variable: "foo".to_owned(),
            value: "soo".to_owned(),
        }));
        assert_eq!(format!("{:?}", parser.next().unwrap().unwrap()), format!("{:?}", AssignmentCommand {
            variable: "do12".to_owned(),
            value: "goo".to_owned(),
        }));
        assert!(parser.next().is_none());
    }

    #[test]
    fn lexer_error_is_passed_through_and_stops_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            fo\"o = soo
            do12 = goo
            ", &context);
        parser.next();

        assert_eq!(ParserError::LexerError(LexerError::UnterminatedQuote { line: 2 }),
                   parser.next().unwrap().err().unwrap());
        assert!(parser.next().is_none());
    }

    #[test]
    fn unknown_command_throws_error_and_stops_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            foo /= soo
            do12 = goo
            ", &context);
        parser.next();

        assert_eq!(ParserError::InvalidCommand(Command {
            line: 2,
            tokens: vec!["foo".to_owned(), "/=".to_owned(), "soo".to_owned()],
        }), parser.next().unwrap().err().unwrap());
        assert!(parser.next().is_none());
    }

    #[test]
    fn invalid_command_throws_error_and_stops_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            12foo = soo
            do12 = goo
            ", &context);
        parser.next();

        assert_eq!(ParserError::InvalidVariableName {
            command: Command {
                line: 2,
                tokens: vec!["12foo".to_owned(), "=".to_owned(), "soo".to_owned()]
            },
            variable: "12foo".to_string(),
        }, parser.next().unwrap().err().unwrap());
        assert!(parser.next().is_none());
    }

    #[derive(Default)]
    struct RemoveVariableCommandSpec {}

    #[derive(Debug)]
    struct RemoveVariableCommand {
        variable: String
    }

    impl ExecutableCommandSpec for RemoveVariableCommandSpec {
        fn validate(&self, command: &Command) -> Result<bool, ParserError> {
            if command.tokens.len() == 1 {
                let mut chars = command.tokens[0].chars();
                return match chars.nth(0) {
                    None => Ok(false),
                    Some(c) => {
                        let variable: String = chars.skip(1).collect();
                        Ok(c == '!' && validate_variable(&variable))
                    }
                }
            }
            Ok(false)
        }

        fn build(&self, command: &mut Command) -> Box<dyn ExecutableCommand> {
            Box::new(RemoveVariableCommand {
                variable: command.tokens[0].chars().skip(1).collect()
            })
        }
    }

    impl ExecutableCommand for RemoveVariableCommand {
        fn execute(&self, context: &mut dyn LexerContext) {
            // do nothing
        }
    }

    #[test]
    fn add_a_new_command() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            !remove_me
            ", &context);
        parser.add_command(Box::new(RemoveVariableCommandSpec::default()));
        parser.next();

        assert_eq!(format!("{:?}", parser.next().unwrap().unwrap()), format!("{:?}", RemoveVariableCommand {
            variable: "remove_me".to_owned()
        }));
    }
}