use std::any::Any;
use std::fmt::{Debug, Display, Formatter};
use log::{Level, debug};
use crate::command::context::Context;
use crate::command::lexer::TokenGroup;
use crate::command::parser::ParserError;
use crate::command::shell::ShellError;

pub trait Command {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError>;
    fn execute(&self, command: &TokenGroup, context: &mut Context)
        -> Result<Option<Box<dyn Any>>, ShellError>;
}

#[derive(PartialEq)]
pub(crate) struct AssignmentCommand {}

#[derive(PartialEq)]
pub(crate) struct DefaultAssignmentCommand {}

#[derive(PartialEq)]
pub(crate) struct SourceCommand {}

impl Command for AssignmentCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError> {
        validate_assigment(command, "=")
    }

    fn execute(&self, command: &TokenGroup, context: &mut Context)
            -> Result<Option<Box<dyn Any>>, ShellError> {
        let var = &command.tokens[0];
        let value = &command.tokens[2];
        debug!("AssignmentCommand: {} = {}", var, value);
        context.set_value(var, value);
        Ok(None)
    }
}
impl Command for DefaultAssignmentCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError> {
        validate_assigment(command, ":=")
    }

    fn execute(&self, command: &TokenGroup, context: &mut Context)
               -> Result<Option<Box<dyn Any>>, ShellError> {
        let var = &command.tokens[0];
        let value = &command.tokens[2];
        if log::log_enabled!(Level::Debug) {
            let replaced_value = context.get_value(var).is_some();
            context.set_default_value(var, value);
            if replaced_value {
                debug!("DefaultAssignmentCommand (new): {} = {}", var, value);
            } else {
                debug!("DefaultAssignmentCommand (replace): {} = {}", var, value);
            }
        } else {
            context.set_default_value(var, value);
        }
        Ok(None)
    }
}

impl Command for SourceCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError> {
        let len = command.tokens.len();
        if len > 0 {
            if command.tokens[0] == "source" {
                if len == 2 || len == 3 && command.tokens[1] == "-s" {
                    return Ok(true);
                }
                return Err(ParserError::InvalidCommandFormat(command.clone()))
            }
        }
        Ok(false)
    }

    fn execute(&self, command: &TokenGroup, context: &mut Context)
               -> Result<Option<Box<dyn Any>>, ShellError> {
        let sub_shell = command.tokens.len() == 3;
        let file = if sub_shell { &command.tokens[3] } else { &command.tokens[2] };
        debug!("loading file: {}, subshell={}", file, sub_shell);
        Ok(None)
    }
}

pub(crate) fn validate_variable(variable: &str) -> bool {
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

fn validate_assigment(command: &TokenGroup, sign: &'static str)
                      -> Result<bool, ParserError> {
    let tokens = &command.tokens;
    if tokens.len() == 3 && tokens[1] == sign {
        if validate_variable(&tokens[0]) {
            Ok(true)
        } else {
            Err(ParserError::InvalidVariableName {
                command: command.to_owned(),
                var: tokens[0].to_owned(),
            })
        }
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod assignment_tests {
    use crate::command::commands::{AssignmentCommand, Command};
    use crate::command::context::Context;
    use crate::command::lexer::TokenGroup;
    use crate::command::parser::ParserError;

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = AssignmentCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = AssignmentCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = AssignmentCommand {};
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&mut command).err().unwrap();

        assert_eq!(ParserError::InvalidVariableName { command, var: "12foo".to_owned() },
                   result);
    }

    #[test]
    fn execute_assignment_command() {
        let mut context = Context::default();
        let spec = AssignmentCommand {};
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };
        spec.validate(&mut command).unwrap();

        spec.execute(&command, &mut context).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
    }
}

#[cfg(test)]
mod default_assignment_tests {
    use crate::command::commands::{DefaultAssignmentCommand, Command};
    use crate::command::context::Context;
    use crate::command::lexer::TokenGroup;
    use crate::command::parser::ParserError;

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = DefaultAssignmentCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = DefaultAssignmentCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = DefaultAssignmentCommand {};
        let command = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ParserError::InvalidVariableName { command, var: "12foo".to_owned() }, result);
    }

    #[test]
    fn execute_default_assignment_command() {
        let mut context = Context::default();
        let spec = DefaultAssignmentCommand {};
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };
        spec.validate(&mut command).unwrap();

        spec.execute(&command, &mut context).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
    }

    #[test]
    fn execute_default_assignment_command_already_there() {
        let mut context = Context::default();
        context.set_value("foo", "soo");
        let spec = DefaultAssignmentCommand {};
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };
        spec.validate(&mut command).unwrap();

        spec.execute(&command, &mut context).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
    }
}
