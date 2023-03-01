use std::fs::File;
use std::io::{BufReader, Write};
use log::{Level, debug};
use crate::command::context::{UserContext, IoContext, CommandContext};
use crate::command::lexer::TokenGroup;
use crate::command::{Path, RegistryError};
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

pub(crate) struct AssignCommand {}

pub(crate) struct CdCommand {}

pub(crate) struct DefaultAssignCommand {}

pub(crate) struct EchoCommand {}

pub(crate) struct MkDirCommand {}

pub(crate) struct SourceCommand {}

pub(crate) struct UnsetCommand {}

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

impl Command for CdCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        if command.tokens[0] == "cd" {
            if command.tokens.len() == 2 {
                Ok(true)
            } else {
                Err(ShellError::InvalidCommandFormat(command.clone()))
            }
        } else {
            Ok(false)
        }
    }

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        match shell.registry.cd(&user_context.pwd, &command.tokens[1]) {
            Ok(path) => {
                user_context.set_pwd(&path.full_path);
                Ok(())
            }
            Err(e) => Err(ShellError::RegistryError {
                command: command.clone(),
                error: e,
            })
        }
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

impl Command for EchoCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        Ok(command.tokens[0] == "echo")
    }

    fn execute(&self,
               command: &TokenGroup,
               _user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        for i in 1..command.tokens.len() {
            io_context.write_str(&command.tokens[i])?;
        }
        io_context.write_str("\n")?;
        Ok(())
    }
}

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
mod assign_tests {
    use std::io;
    use crate::command::commands::{AssignCommand, Command};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let command = AssignCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let command = AssignCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let command = AssignCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidVariableName { command: tokens, var: "12foo".to_owned() },
                   result);
    }

    #[test]
    fn execute_assignment_command() {
        let mut context = UserContext::default();
        let command = AssignCommand {};
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let command_context = CommandContext::new();
        let mut source = IoContext::new("test", &mut input, &mut output);
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        };
        command.validate(&tokens).unwrap();

        command.execute(&tokens, &mut context, &mut source, &command_context, &mut shell).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
    }
}

#[cfg(test)]
mod default_assign_tests {
    use std::io;
    use crate::command::commands::{DefaultAssignCommand, Command};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let command = DefaultAssignCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let command = DefaultAssignCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let command = DefaultAssignCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["12foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidVariableName {
            command: tokens,
            var: "12foo".to_owned(),
        }, result);
    }

    #[test]
    fn execute_default_assignment_command_sets_new_value() {
        let mut context = UserContext::default();
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut source = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::new();
        let command = DefaultAssignCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };
        command.validate(&tokens).unwrap();

        command.execute(&tokens, &mut context, &mut source, &command_context, &mut shell).unwrap();

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
        let command = DefaultAssignCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()],
        };
        command.validate(&tokens).unwrap();

        command.execute(&tokens, &mut context, &mut source, &command_context, &mut shell).unwrap();

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
        let command = UnsetCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "foo".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_valid_unset_command_with_multiple_variables_returns_true() {
        let command = UnsetCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "foo".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn different_command_returns_false() {
        let command = UnsetCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["foo".to_owned(), "=".to_owned(), "unset".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn unset_with_no_variables_is_error() {
        let command = UnsetCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidCommandFormat(tokens), result);
    }

    #[test]
    fn unset_invalid_variable_name_returns_error() {
        let command = UnsetCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "foo".to_owned(), "12foo".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidVariableName { command: tokens, var: "12foo".to_owned() },
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
        let command = UnsetCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["unset".to_owned(), "soo".to_owned(), "goo".to_owned()],
        };

        command.execute(&tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
        assert!(context.get_value("soo").is_none());
        assert!(context.get_value("goo").is_none());
    }
}

#[cfg(test)]
mod mkdir_tests {
    use std::io;
    use crate::command::commands::{Command, MkDirCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_mkdir_command_returns_true() {
        let command = MkDirCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned(), "foo".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_mkdir_command_without_arg_returns_error() {
        let command = MkDirCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidCommandFormat(tokens), result);
    }

    #[test]
    fn validate_mkdir_command_with_multiple_args_returns_error() {
        let command = MkDirCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned(), "foo".to_owned(), "bar".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidCommandFormat(tokens), result);
    }

    #[test]
    fn mkdir_creates_new_directories() {
        let mut context = UserContext::default();
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::new();
        let command = MkDirCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned(), "foo/bar".to_owned()],
        };

        command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        let path = shell.registry.path("/foo/bar").unwrap();
        assert_eq!("/foo/bar", path.full_path);
    }
}

#[cfg(test)]
mod echo_tests {
    use std::io;
    use crate::command::commands::{Command, EchoCommand, MkDirCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_echo_command_returns_true() {
        let command = EchoCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["echo".to_owned(), "foo".to_owned(), "bar".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_echo_command_without_args_returns_true() {
        let command = EchoCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned()],
        });

        assert!(result);
    }

    #[test]
    fn non_echo_command_returns_false() {
        let command = EchoCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned(), "foo".to_owned()],
        });

        assert!(!result);
    }

    #[test]
    fn echo_command_writes_expanded_tokens_to_output() {
        let mut context = UserContext::default();
        let mut shell = Shell::new();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::new();
        let command = MkDirCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned(), "foo/bar".to_owned()],
        };

        command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        let path = shell.registry.path("/foo/bar").unwrap();
        assert_eq!("/foo/bar", path.full_path);
    }
}