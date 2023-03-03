use std::fs::File;
use std::io::BufReader;
use log::{Level, debug};
use crate::command::context::{UserContext, IoContext, CommandContext};
use crate::command::lexer::TokenGroup;
use crate::command::oso::PolarValue;
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

pub(crate) struct CreateCommand {}

pub(crate) struct DefaultAssignCommand {}

pub(crate) struct EchoCommand {}

pub(crate) struct ExecuteCommand {}

pub(crate) struct LsCommand {}

pub(crate) struct MkDirCommand {}

pub(crate) struct PwdCommand {}

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
                user_context.set_pwd(path.abs_path());
                Ok(())
            }
            Err(e) => Err(ShellError::RegistryCommandError {
                command: command.clone(),
                error: e,
            })
        }
    }
}

impl Command for CreateCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        if command.tokens[0] == "create" {
            if command.tokens.len() >= 3 {
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
        let mut args: Vec<&str> = vec![];
        for i in 3..command.tokens.len() {
            args.push(&command.tokens[i]);
        }

        shell.registry.parsed_create_instance(&user_context.pwd,
                                              &command.tokens[1],
                                              &command.tokens[2],
                                              &args).map_err(|e|
            ShellError::RegistryCommandError {
                command: command.clone(),
                error: e,
            })
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
            if i != 1 {
                io_context.write_str(" ")?;
            }
            io_context.write_str(&command.tokens[i])?;
        }
        Ok(())
    }
}

impl Command for ExecuteCommand {
    fn validate(&self, _command: &TokenGroup) -> Result<bool, ShellError> {
        Ok(true)
    }

    fn execute(&self,
               command: &TokenGroup,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        let mut args: Vec<&str> = vec![];
        for i in 1..command.tokens.len() {
            args.push(&command.tokens[i]);
        }

        let result = shell.registry.parsed_invoke_method(&user_context.pwd,
                                                         &command.tokens[0],
                                                         &args)
            .map_err(|e| ShellError::RegistryCommandError {
                command: command.clone(),
                error: e,
            })?;

        write_object(io_context, shell, &result)?;
        Ok(())
    }
}

impl Command for LsCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        if command.tokens[0] == "ls" {
            if command.tokens.len() == 1 || command.tokens.len() == 2 {
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
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        let registry = &shell.registry;
        let mut children: Vec<String> = vec![];

        let cd = if command.tokens.len() == 1 { "." } else { &command.tokens[1] };

        let path = registry.cd(&user_context.pwd, cd).map_err(|e| ShellError::RegistryError(e))?;

        //for (child_name, child_id) in &path.children {
        //    let child = registry.paths.get(&child_id).unwrap();

        for child in path.children(&registry) {
            let mut child_str = String::new();
            child_str.push_str(child.name());
            if child.has_children() {
                child_str.push('/');
            }

            if child.instance().is_some() {
                let instance = child.instance().unwrap();
                let class = registry.instance_class(instance);

                child_str.push(' ');
                child_str.push_str(&class.name);
            } else if child.method.is_some() {
                let instance = child.owner_instance(&registry).unwrap();
                let class = registry.instance_class(instance);
                let method_name = child.method.unwrap();
                let method = class.instance_methods.get(method_name).unwrap();

                child_str.push_str("! ");
                child_str.push_str(&class.name);
                child_str.push_str("::");
                child_str.push_str(method_name);
                child_str.push('(');
                let mut first = true;
                for pt in method.param_types() {
                    if !first {
                        child_str.push_str(", ");
                    }
                    first = false;
                    child_str.push_str(pt);
                }
                child_str.push(')');

            } else if child.attr.is_some() {
                let instance = child.owner_instance(&registry).unwrap();
                let class = registry.instance_class(instance);
                let attr_name = child.attr.unwrap();

                child_str.push('+');
                child_str.push_str(&class.name);
                child_str.push('.');
                child_str.push_str(attr_name);
            }

            child_str.push('\n');
            children.push(child_str);
        }

        children.sort();
        for child in children {
            io_context.write_string(child)?;
        }

        Ok(())
    }
}

fn write_object(io_context: &mut IoContext, shell: &Shell, result: &PolarValue)
                -> Result<(), ShellError> {
    match result {
        PolarValue::Integer(i) => io_context.write_string(format!("{}", i)),
        PolarValue::Float(f) => io_context.write_string(format!("{}", f)),
        PolarValue::String(s) => io_context.write_string(format!("\"{}\"", s)),
        PolarValue::Boolean(b) => io_context.write_string(format!("{}", b)),
        PolarValue::Map(m) => {
            io_context.write_str("{{")?;
            let mut first = true;
            for (key, val) in m {
                if !first {
                    io_context.write_str(",")?;
                }
                first = false;
                io_context.write_string(format!("\"{}\":", key))?;
                write_object(io_context, shell, val)?;
            }
            io_context.write_str("}}")
        },
        PolarValue::List(l) => {
            io_context.write_str("[")?;
            for i in 0..l.len() {
                if i != 0 {
                    io_context.write_str(",")?;
                }
                write_object(io_context, shell, &l[i])?;
            }
            io_context.write_str("]")
        },
        PolarValue::Instance(i) => {
            let clz = shell.registry.instance_class(i);
            io_context.write_str("{{")?;
            let mut first = true;
            for (attr_name, attr) in &clz.attributes {
                if !first {
                    io_context.write_str(",")?;
                }
                first = false;
                io_context.write_string(format!("\"{}\":", attr_name))?;

                let value = shell.registry.instance_attr(i, attr);
                write_object(io_context, shell, &value)?;
            }
            io_context.write_str("}}")
        },
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
            |e| ShellError::RegistryCommandError {
                command: command.clone(),
                error: e,
            })
    }
}

impl Command for PwdCommand {
    fn validate(&self, command: &TokenGroup) -> Result<bool, ShellError> {
        if command.tokens[0] == "pwd" {
            if command.tokens.len() == 1 {
                Ok(true)
            } else {
                Err(ShellError::InvalidCommandFormat(command.clone()))
            }
        } else {
            Ok(false)
        }
    }

    fn execute(&self,
               _command: &TokenGroup,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        io_context.write_str(&user_context.pwd)
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
        let mut shell = Shell::default();
        let mut input = io::stdin();
        let mut output = io::sink();
        let command_context = CommandContext::default();
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
        let mut shell = Shell::default();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut source = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
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
        let mut shell = Shell::default();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut source = IoContext::new("test", &mut input, &mut output);

        let command_context = CommandContext::default();
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
        let mut shell = Shell::default();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
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
        let mut shell = Shell::default();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
        let command = MkDirCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned(), "foo/bar".to_owned()],
        };

        command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        let path = shell.registry.path("/foo/bar").unwrap();
        assert_eq!("/foo/bar", path.abs_path());
    }
}

#[cfg(test)]
mod cd_tests {
    use std::io;
    use crate::command::commands::{Command, CdCommand, MkDirCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};

    #[test]
    fn validate_valid_cd_command_returns_error() {
        let command = CdCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["cd".to_owned(), "foo".to_owned(), "bar".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidCommandFormat(tokens), result);
    }

    #[test]
    fn validate_cd_command_without_arg_returns_error() {
        let command = CdCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["cd".to_owned()],
        };

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(ShellError::InvalidCommandFormat(tokens), result);
    }

    #[test]
    fn validate_cd_command_with_one_arg_returns_true() {
        let command = CdCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["cd".to_owned(), "foo/bar".to_owned()],
        };

        let result = command.validate(&tokens).unwrap();

        assert!(result);
    }

    #[test]
    fn cd_navigates_to_new_directory() {
        let mut context = UserContext::default();
        let mut shell = Shell::default();
        let mut input = io::stdin();
        let mut output = io::sink();
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
        MkDirCommand {}.execute(
            &TokenGroup {
                line: 0,
                tokens: vec!["mkdir".to_owned(), "foo/bar/soo".to_owned()],
            }, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        CdCommand {}.execute(
            &TokenGroup {
                line: 0,
                tokens: vec!["cd".to_owned(), "foo/bar".to_owned()],
            }, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        assert_eq!("/foo/bar", context.pwd);
    }
}

#[cfg(test)]
mod echo_tests {
    use std::io;
    use std::io::Cursor;
    use crate::command::commands::{Command, EchoCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::Shell;

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
            tokens: vec!["echo".to_owned()],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn non_echo_command_returns_false() {
        let command = EchoCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec!["mkdir".to_owned(), "foo".to_owned()],
        }).unwrap();

        assert!(!result);
    }

    #[test]
    fn echo_command_writes_tokens_to_output() {
        let mut context = UserContext::default();
        let mut shell = Shell::default();
        let mut input = io::stdin();
        let mut vec: Vec<u8> = Vec::new();
        let mut output = Cursor::new(&mut vec);
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
        let command = EchoCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec!["echo".to_owned(), "foo".to_owned(), "me".to_owned()],
        };

        command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        let str = String::from_utf8(vec).unwrap();
        assert_eq!("foo me", str);
    }
}

#[cfg(test)]
mod create_tests {
    use std::io;
    use std::io::Cursor;
    use crate::command::commands::{Command, CreateCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};
    use crate::command::shell::ShellError::InvalidCommandFormat;
    use crate::command::oso::PolarClass;
    use crate::command::RegistryError;

    #[derive(Clone, PolarClass)]
    struct User {
        #[polar(attribute)]
        pub username: String,
        #[polar(attribute)]
        pub user_id: i32,
    }

    impl User {
        fn new(username: String, user_id: i32) -> User {
            User { username, user_id }
        }
    }

    #[test]
    fn validate_valid_create_command_returns_true() {
        let command = CreateCommand {};

        let result = command.validate(&TokenGroup {
            line: 0,
            tokens: vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
                "core::command::commands::create_tests::User".to_owned(),
                "jgreco".to_owned(),
                "42".to_owned()
            ],
        }).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_create_command_with_1_param_is_error() {
        let command = CreateCommand {};
        let token = TokenGroup {
            line: 0,
            tokens: vec![
                "create".to_owned()
            ],
        };

        let result = command.validate(&token).err().unwrap();

        assert_eq!(result, InvalidCommandFormat(token));
    }

    #[test]
    fn validate_create_command_with_2_params_returns_false() {
        let command = CreateCommand {};
        let token = TokenGroup {
            line: 0,
            tokens: vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
            ],
        };

        let result = command.validate(&token).err().unwrap();

        assert_eq!(result, InvalidCommandFormat(token));
    }

    #[test]
    fn execute_create_command() {
        let mut context = UserContext::default();
        let mut shell = Shell::default();
        shell.cache_class(User::get_polar_class_builder()
            .set_constructor(User::new, vec!["string", "int"])
            .build()).unwrap();
        let mut input = io::stdin();
        let mut vec: Vec<u8> = Vec::new();
        let mut output = Cursor::new(&mut vec);
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
        let command = CreateCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
                "rcore::command::commands::create_tests::User".to_owned(),
                "jgreco".to_owned(),
                "42".to_owned()
            ],
        };

        command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        let user = shell.registry.instance_value::<User>("/foo/bar", ".").unwrap();
        assert_eq!(42, user.user_id);
        assert_eq!("jgreco", user.username);
    }

    #[test]
    fn execute_create_command_with_wrong_data_type_is_an_era() {
        let mut context = UserContext::default();
        let mut shell = Shell::default();
        shell.cache_class(User::get_polar_class_builder()
            .set_constructor(User::new, vec!["int", "string"])
            .build()).unwrap();
        let mut input = io::stdin();
        let mut vec: Vec<u8> = Vec::new();
        let mut output = Cursor::new(&mut vec);
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
        let command = CreateCommand {};
        let tokens = TokenGroup {
            line: 0,
            tokens: vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
                "rcore::command::commands::create_tests::User".to_owned(),
                "jgreco".to_owned(),
                "42".to_owned()
            ],
        };

        let result = command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).err().unwrap();

        assert_eq!(result, ShellError::RegistryCommandError {
            command: tokens,
            error: RegistryError::InvalidMethodParameter {
                class: "rcore::command::commands::create_tests::User".to_owned(),
                method: "<constructor>".to_owned(),
                param_index: 0,
                param_type: "int",
                reason: "",
            },
        })
    }
}

#[cfg(test)]
mod execute_tests {
    use std::io;
    use std::io::Cursor;
    use crate::command::commands::{Command, CreateCommand, ExecuteCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::TokenGroup;
    use crate::command::shell::{Shell, ShellError};
    use crate::command::oso::PolarClass;
    use crate::command::RegistryError;

    #[derive(Clone, PolarClass)]
    struct User {
        #[polar(attribute)]
        pub username: String,
        #[polar(attribute)]
        pub user_id: i32,
    }

    impl User {
        fn new(username: String, user_id: i32) -> User {
            User { username, user_id }
        }

        pub fn add_one(&self, id: i32) -> i32 {
            id + 1
        }
    }

    #[test]
    fn execute_method() {
        let mut context = UserContext::default();
        let mut shell = Shell::default();
        shell.cache_class(User::get_polar_class_builder()
            .set_constructor(User::new, vec!["string", "int"])
            .add_method("add_one", User::add_one, vec!["int"], Some("add"))
            .build()).unwrap();
        let mut input = io::stdin();
        let mut vec: Vec<u8> = Vec::new();
        let mut output = Cursor::new(&mut vec);
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
        CreateCommand {}.execute(
            &TokenGroup {
                line: 0,
                tokens: vec![
                    "create".to_owned(),
                    "/foo/bar".to_owned(),
                    "rcore::command::commands::execute_tests::User".to_owned(),
                    "jgreco".to_owned(),
                    "42".to_owned()
                ],
            }, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        ExecuteCommand {}.execute(
            &TokenGroup {
                line: 0,
                tokens: vec![
                    "/foo/bar/add".to_owned(),
                    "41".to_owned()
                ],
            }, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        let string = String::from_utf8(vec).unwrap();
        assert_eq!("42", string);
    }

    #[test]
    fn execute_method_with_incorrect_data_type_is_error() {
        let mut context = UserContext::default();
        let mut shell = Shell::default();
        shell.cache_class(User::get_polar_class_builder()
            .set_constructor(User::new, vec!["string", "int"])
            .add_method("add_one", User::add_one, vec!["int"], Some("add"))
            .build()).unwrap();
        let mut input = io::stdin();
        let mut vec: Vec<u8> = Vec::new();
        let mut output = Cursor::new(&mut vec);
        let mut io_context = IoContext::new("test", &mut input, &mut output);
        let command_context = CommandContext::default();
        CreateCommand {}.execute(
            &TokenGroup {
                line: 0,
                tokens: vec![
                    "create".to_owned(),
                    "/foo/bar".to_owned(),
                    "rcore::command::commands::execute_tests::User".to_owned(),
                    "jgreco".to_owned(),
                    "42".to_owned()
                ],
            }, &mut context, &mut io_context, &command_context, &mut shell).unwrap();
        let command = TokenGroup {
            line: 0,
            tokens: vec![
                "/foo/bar/add".to_owned(),
                "foo".to_owned()
            ],
        };

        let err = ExecuteCommand {}.execute(
            &command, &mut context, &mut io_context, &command_context, &mut shell).err().unwrap();

        assert_eq!(ShellError::RegistryCommandError {
            command: command,
            error: RegistryError::InvalidMethodParameter {
                class: "User".to_owned(),
                method: "add_one".to_string(),
                param_index: 0,
                param_type: "int",
                reason: "",
            }
        }, err);
    }
}