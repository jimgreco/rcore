use std::fmt::{Debug, Display, Formatter};
use crate::command::lexer::{LexerContext, TokenGroup};
use crate::command::parser::ParserError;

pub trait ExecutableCommandSpec {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError>;
    fn build(&self, command: &mut TokenGroup) -> Box<dyn ExecutableCommand>;
}

pub trait ExecutableCommand: Debug {
    fn execute(&self, context: &mut dyn LexerContext);
}

#[derive(Default)]
pub(crate) struct AssignmentCommandSpec {}

#[derive(Debug, PartialEq)]
pub(crate) struct AssignmentCommand {
    pub(crate) variable: String,
    pub(crate) value: String
}

#[derive(Default)]
pub(crate) struct DefaultAssignmentCommandSpec {}

#[derive(Debug, PartialEq)]
pub(crate) struct DefaultAssignmentCommand {
    pub(crate) variable: String,
    pub(crate) value: String
}

impl ExecutableCommandSpec for AssignmentCommandSpec {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError> {
        validate_assigment(command, "=")
    }

    fn build(&self, command: &mut TokenGroup) -> Box<dyn ExecutableCommand> {
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
    fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError> {
        validate_assigment(command, ":=")
    }

    fn build(&self, command: &mut TokenGroup) -> Box<dyn ExecutableCommand> {
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

pub fn validate_variable(variable: &str) -> bool {
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
                variable: tokens[0].to_owned(),
            })
        }
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod assignment_tests {
    use crate::command::commands::{AssignmentCommandSpec, ExecutableCommandSpec};
    use crate::command::lexer::TokenGroup;
    use crate::command::parser::ParserError;

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = AssignmentCommandSpec::default();

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = AssignmentCommandSpec::default();

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = AssignmentCommandSpec::default();
        let command = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ParserError::InvalidVariableName { command, variable: "12foo".to_owned() }, result);
    }
}

#[cfg(test)]
mod default_assignment_tests {
    use crate::command::commands::{DefaultAssignmentCommandSpec, ExecutableCommandSpec};
    use crate::command::lexer::TokenGroup;
    use crate::command::parser::ParserError;

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = DefaultAssignmentCommandSpec::default();

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = DefaultAssignmentCommandSpec::default();

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = DefaultAssignmentCommandSpec::default();
        let command = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ParserError::InvalidVariableName { command, variable: "12foo".to_owned() }, result);
    }
}
