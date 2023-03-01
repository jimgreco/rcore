use std::fs::File;
use std::io::{BufReader};
use log::{Level, debug};
use crate::command::context::{UserContext, IoContext, CommandContext};
use crate::command::lexer::TokenGroup;
use crate::command::shell::{Shell, ShellError};

pub trait Command {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError>;

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError>;
}

#[derive(PartialEq)]
pub(crate) struct AssignCommand {}

#[derive(PartialEq)]
pub(crate) struct DefaultAssignCommand {}

#[derive(PartialEq)]
pub(crate) struct UnsetCommand {}

#[derive(PartialEq)]
pub(crate) struct SourceCommand {}

#[derive(PartialEq)]
pub(crate) struct MkDirCommand {}

#[derive(PartialEq)]
pub(crate) struct CdCommand {}

impl Command for MkDirCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        if command.tokens[0] == "mkdir" {
            return if command.tokens.len() == 2 {
                Ok(true)
            } else {
                Err(ShellError::InvalidCommandFormat(command.clone()))
            };
        }
        Ok(false)
    }

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        shell.registry.mkdir(&user_context.pwd, &command.tokens[1]).map_err(
            |e| ShellError::RegistryError {
                command: command.clone(),
                error: e,
            })
    }
}

impl Command for AssignCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        validate_assigment(command, "=")
    }

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        let var = &command.tokens[0];
        let value = &command.tokens[2];
        debug!("[Assign] setting variable {} = {}", var, value);
        user_context.set_value(var, value);
        Ok(())
    }
}

impl Command for DefaultAssignCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        validate_assigment(command, ":=")
    }

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        let var = &command.tokens[0];
        let value = &command.tokens[2];
        if log::log_enabled!(Level::Debug) {
            let replaced_value = user_context.get_value(var).is_some();
            user_context.set_default_value(var, value);
            if replaced_value {
                debug!("[DefaultAssign] setting variable {} = {}", var, value);
            } else {
                debug!("[DefaultAssign] replacing variable {} = {}", var, value);
            }
        } else {
            user_context.set_default_value(var, value);
        }
        Ok(())
    }
}

impl Command for UnsetCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        if &command.tokens[0] == "unset" {
            let len = command.tokens.len();
            if len == 1 {
                return Err(ShellError::InvalidCommandFormat(command.clone()));
            }
            for i in 1..len {
                if !validate_variable(&command.tokens[i]) {
                    return Err(ShellError::InvalidVariableName {
                        command: command.clone(),
                        var: command.tokens[i].to_owned(),
                    });
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        debug!("[Unset] removing variables {}",
            command.tokens_substring(1, command.tokens.len()));
        for i in 1..command.tokens.len() {
            user_context.remove_value(&command.tokens[i]);
        }
        Ok(())
    }
}

impl Command for SourceCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        let len = command.tokens.len();
        if command.tokens[0] == "source" {
            return if len == 1 {
                Err(ShellError::InvalidCommandFormat(command.clone()))
            } else if len == 2 && command.tokens[1] != "-s" || len >= 3 {
                Ok(true)
            } else {
                Err(ShellError::InvalidCommandFormat(command.clone()))
            };
        }
        Ok(false)
    }

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        let subshell = command.tokens[1] == "-s";
        let arg_start = if subshell { 3 } else { 2 };
        let file_name = &command.tokens[arg_start - 1];
        let args = &command.tokens[arg_start..];

        debug!("[Source] loading file {}{}, args={}",
            file_name,
            if subshell { " (subshell)" } else { "" },
            command.tokens_substring(arg_start, command.tokens.len()));

        // make a copy of all variables
        let mut new_user_context = user_context.clone();
        // add our arguments to the file
        new_user_context.clear_arguments();
        for arg in args {
            new_user_context.add_argument(arg);
        }

        // load file
        match File::open(file_name) {
            Ok(f) => {
                let mut reader = BufReader::new(f);
                // the new I/O context has the new file, but the same output
                let mut new_io_context = IoContext::new(
                    file_name, &mut reader, &mut io_context.output);
                shell.execute_commands(
                    &mut new_user_context, &mut new_io_context, _command_context)?;
            }
            Err(error) => return Err(ShellError::UnknownFile {
                file: file_name.to_owned(),
                error,
            })
        }

        // update variables if we're not a subshell
        if !subshell {
            user_context.set_pwd(&new_user_context.pwd);
            user_context.clear_variables();
            for (key, value) in &new_user_context.variables {
                user_context.set_value(key, value);
            }
        }

        Ok(())
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

fn validate_assigment(command: &TokenGroup, sign: &'static str)
                      -> Result<bool, ShellError> {
    let tokens = &command.tokens;
    if tokens.len() == 3 && tokens[1] == sign {
        if validate_variable(&tokens[0]) {
            Ok(true)
        } else {
            Err(ShellError::InvalidVariableName {
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
    use std::io;
    use crate::command::commands::{AssignCommand, Command};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = AssignCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = AssignCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = AssignCommand {};
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&mut command).err().unwrap();

        assert_eq!(ShellError::InvalidVariableName { command, var: "12foo".to_owned() },
                   result);
    }

    #[test]
    fn execute_assignment_command() {
        let mut context = UserContext::default();
        let spec = AssignCommand {};
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let command_context = CommandContext::new();
        let mut source = IoContext::new("test", &mut input, &mut output);
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };
        spec.validate(&mut command).unwrap();

        spec.execute(&command, &mut context, &mut source, &command_context, &mut shell).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
    }
}

#[cfg(test)]
mod default_assignment_tests {
    use std::io;
    use crate::command::commands::{DefaultAssignCommand, Command};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let spec = DefaultAssignCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let spec = DefaultAssignCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let spec = DefaultAssignCommand {};
        let command = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ShellError::InvalidVariableName { command, var: "12foo".to_owned() }, result);
    }

    #[test]
    fn execute_default_assignment_command_sets_new_value() {
        let mut context = UserContext::default();
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut source = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::new();
        let spec = DefaultAssignCommand {};
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };
        spec.validate(&mut command).unwrap();

        spec.execute(&command, &mut context, &mut source, &command_context, &mut shell).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
    }

    #[test]
    fn execute_default_assignment_command_already_there_doesnt_replace() {
        let mut context = UserContext::default();
        context.set_value("foo", "soo");
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut source = IoContext::new("test", &mut input, &mut output);

        let command_context = CommandContext::new();
        let spec = DefaultAssignCommand {};
        let mut command = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };
        spec.validate(&mut command).unwrap();

        spec.execute(&command, &mut context, &mut source, &command_context, &mut shell).unwrap();

        assert_eq!("soo", context.get_value("foo").unwrap());
    }
}

#[cfg(test)]
mod unset_tests {
    use std::io;
    use crate::command::commands::{Command, UnsetCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_unset_command_with_one_variable_returns_true() {
        let spec = UnsetCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "foo".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_valid_unset_command_with_multiple_variables_returns_true() {
        let spec = UnsetCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "foo".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn different_command_returns_false() {
        let spec = UnsetCommand {};

        let result = spec.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "unset".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn unset_with_no_variables_is_error() {
        let spec = UnsetCommand {};
        let command = TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ShellError::InvalidCommandFormat(command), result);
    }

    #[test]
    fn unset_invalid_variable_name_returns_error() {
        let spec = UnsetCommand {};
        let command = TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "foo".to_owned(), "12foo".to_owned()],
        };

        let result = spec.validate(&command).err().unwrap();

        assert_eq!(ShellError::InvalidVariableName { command, var: "12foo".to_owned() },
                   result);
    }

    #[test]
    fn execute_unset_command() {
        let mut context = UserContext::default();
        context.set_value("foo", "bar");
        context.set_value("soo", "doo");
        context.set_value("goo", "boo");
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::new();
        let spec = UnsetCommand {};
        let command = TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "soo".to_owned(), "goo".to_owned()],
        };

        spec.execute(&command, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
        assert!(context.get_value("soo").is_none());
        assert!(context.get_value("goo").is_none());
    }
}