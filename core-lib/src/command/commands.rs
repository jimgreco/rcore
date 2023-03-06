use std::fs::File;
use std::io;
use std::io::BufReader;
use std::ptr::eq;
use log::{Level, debug};
use crate::command::context::{UserContext, IoContext, CommandContext};
use crate::command::lexer::Tokens;
use crate::command::oso::PolarValue;
use crate::command::ShellError;
use crate::command::shell::Shell;

use thiserror::Error;

/// Errors thrown while validating commands.
#[derive(Debug, Error, PartialEq)]
pub enum CommandValidationError {
    #[error("invalid command format, expected: {format}")]
    InvalidCommandFormat {
        format: &'static str
    },
    #[error("invalid variable name: {0}")]
    InvalidVariableName(String)
}

/// Errors thrown while executing commands.
#[derive(Debug, Error)]
pub enum CommandExecutionError {
    #[error("File does not exist: {file}, error={error}")]
    UnableToOpenFile {
        file: String,
        error: io::Error,
    },
    #[error("invalid variable name: {var}, command={tokens}")]
    InvalidVariableName {
        tokens: Tokens,
        var: String,
    },
    #[error("source command invoked too many times recursively: {0}")]
    MaxSourceCommand(usize),
}

impl PartialEq for CommandExecutionError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CommandExecutionError::UnableToOpenFile { file, ..},
            CommandExecutionError::UnableToOpenFile { file: file2, .. }) => file == file2,
            (CommandExecutionError::InvalidVariableName { tokens: command, var },
                CommandExecutionError::InvalidVariableName { tokens: command2, var: var2 })
            => command == command2 && var == var2,
            (CommandExecutionError::MaxSourceCommand(size),
                CommandExecutionError::MaxSourceCommand(size2)) => size == size2,
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !eq(self, other)
    }
}

/// A command is an operation that can be executed in the Shell.
pub trait Command {
    /// Returns the name of the command.
    fn name(&self) -> &'static str;

    /// Returns true if the implementation can execute a command with the specified tokens.
    ///
    /// A shell error is returned if the command is improperly formatted.
    fn validate(&self, tokens: &Tokens) -> Result<bool, CommandValidationError>;

    /// Executes a command with the specified context.
    ///
    /// The implementation can assume that the command has been previously validated.
    ///
    /// An error is returned if there was an issue executing the command.
    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError>;
}

/// Assigns a value to a variable.
///
/// # Example
/// ```
/// let (_, context) = rcore::command::Shell::from_string("foo = bar").unwrap();
///
/// assert_eq!("bar", context.get_value("foo").unwrap());
/// ```
pub struct AssignCommand {}

/// Changes the current working directory.
///
/// # Example
/// ```
/// let (_, context) = rcore::command::Shell::from_string(
///     "mkdir /foo/bar/me
///      cd /foo/bar/me
///      cd ../..").unwrap();
///
/// assert_eq!("/foo", context.pwd());
/// ```
pub struct CdCommand {}

/// Creates an instance of a struct.
pub struct CreateCommand {}

/// Assigns a value to a variable if it is not already assigned.
///
/// # Example
/// ```
/// let (_, context) = rcore::command::Shell::from_string(
///     "v1 = abc                # v1 = abc
///      v1 := def               # v1 = abc (already has value, not overridden)
///      v2 := hij               # v2 = hij
///      v2 = klm                # v2 = klm (overridden)
///      v3 := \"nop   qrs\"     # v3 = nop, quotes allow for spaces in values
///      echo $v1 $v2 $v3").unwrap();
///
/// assert_eq!("abc", context.get_value("v1").unwrap());
/// assert_eq!("klm", context.get_value("v2").unwrap());
/// assert_eq!("nop   qrs", context.get_value("v3").unwrap());
/// ```
pub struct DefaultAssignCommand {}

/// Writes the arguments back to the output.
///
/// # Example
/// ```
/// let (result, _) = rcore::command::Shell::from_string(
///     "v1 = abc
///      v2 := hij
///      v3 := \"nop   qrs\"
///      echo $v1 $v2 $v3").unwrap();
///
/// assert_eq!("abc hij nop   qrs", result);
/// ```
pub struct EchoCommand {}

/// Invokes a method or retrieves the value of an attribute on an instance.
pub struct ExecuteCommand {}

/// Lists the contents of a directory
///
/// # Example
/// ```
/// let (result, _) = rcore::command::Shell::from_string(
///     "mkdir /foo/bar
///      cd /foo
///      ls").unwrap();
///
/// assert_eq!("bar\n", result);
/// ```
pub struct LsCommand {}

/// Creates a new directory.
///
/// This will create directories recursively if they do not exist.
///
/// # Example
/// ```
/// let (_, context) = rcore::command::Shell::from_string(
///     "mkdir /foo
///      mkdir /foo/bar/me
///      cd foo/bar/me").unwrap();
///
/// assert_eq!("/foo/bar/me", context.pwd());
/// ```
pub struct MkDirCommand {}

/// Returns the current working directory.
///
/// # Example
/// ```
/// let (result, _) = rcore::command::Shell::from_string(
///     "mkdir /foo/bar/me
///      cd /foo/bar/me
///      cd ../..
///      pwd").unwrap();
///
/// assert_eq!("/foo", result);
/// ```
pub struct PwdCommand {}

/// Returns the current working directory.
///
/// # Examples
/// ```
/// use std::fs::File;
/// use std::io::Write;
///
/// let mut file = File::create("/tmp/subshell_test.commands").unwrap();
/// file.write("v1 = foo
///             v2 = bar".as_bytes()).unwrap();
///
/// let (_, context) = rcore::command::Shell::from_string(
///     "v1 = hello
///      source /tmp/subshell_test.commands").unwrap();
///
/// assert_eq!("foo", context.get_value("v1").unwrap());
/// assert_eq!("bar", context.get_value("v2").unwrap());
/// ```
/// The subshell option ('-s') can be used to isolate changes to the user's context.
/// ```
/// use std::fs::File;
/// use std::io::Write;
///
/// let mut file = File::create("/tmp/subshell_test.commands").unwrap();
/// file.write("v1 = foo
///             v2 = bar".as_bytes()).unwrap();
///
/// let (_, context) = rcore::command::Shell::from_string(
///     "v1 = hello
///      source -s /tmp/subshell_test.commands").unwrap();
///
/// assert_eq!("hello", context.get_value("v1").unwrap());
/// assert_eq!(None, context.get_value("v2"));
/// ```
pub struct SourceCommand {}

/// Removes a variable.
///
/// # Example
/// ```
/// let (result, context) = rcore::command::Shell::from_string(
///     "v1 = abc
///      echo $v1
///      unset v1").unwrap();
///
/// assert_eq!("abc", result);
/// assert_eq!(None, context.get_value("v1"));
/// ```
pub struct UnsetCommand {}

impl Command for AssignCommand {
    fn name(&self) -> &'static str {
        "assign"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        validate_assigment(command, "=")
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        let var = &tokens.get(0);
        let value = &tokens.get(2);
        debug!("[Assign] setting variable {} = {}", var, value);
        user_context.set_value(var, value);
        Ok(())
    }
}

impl Command for CdCommand {
    fn name(&self) -> &'static str {
        "cd"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        if command.get(0) == "cd" {
            return if command.len() == 2 {
                Ok(true)
            } else {
                Err(CommandValidationError::InvalidCommandFormat {
                    format: "cd <dir>"
                })
            }
        }
        Ok(false)
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        match shell.registry.cd(&user_context.pwd, &tokens.get(1)) {
            Ok(path) => {
                debug!("[Cd] setting current working directory = {}", path.abs_path());
                user_context.set_pwd(path.abs_path());
                Ok(())
            }
            Err(e) => Err(ShellError::RegistryError {
                src: io_context.to_source_info(),
                tokens: tokens.clone(),
                error: e,
            })
        }
    }
}

impl Command for CreateCommand {
    fn name(&self) -> &'static str {
        "create"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        if command.get(0) == "create" {
            return if command.len() >= 3 {
                Ok(true)
            } else {
                Err(CommandValidationError::InvalidCommandFormat {
                    format: "create <dir> <struct> [args ...]",
                })
            }
        }
        Ok(false)
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        let mut args: Vec<&str> = vec![];
        for i in 3..tokens.len() {
            args.push(&tokens.get(i));
        }

        debug!("[Create] creating instance: dir={}, class={}, args=[{}]",
            &tokens.get(1), &tokens.get(2), &args.join(", "));
        shell.registry.parsed_create_instance(
            &user_context.pwd, &tokens.get(1), &tokens.get(2), &args
        ).map_err(|e| ShellError::RegistryError {
            src: io_context.to_source_info(),
            tokens: tokens.clone(),
            error: e,
        })
    }
}

impl Command for DefaultAssignCommand {
    fn name(&self) -> &'static str {
        "default_assign"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        validate_assigment(command, ":=")
    }

    fn execute(&self,
               command: &Tokens,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        let var = &command.get(0);
        let value = &command.get(2);
        if log::log_enabled!(Level::Debug) {
            let replaced_value = user_context.get_value(var);
            if replaced_value.is_none() {
                debug!("[DefaultAssign] setting variable {} = {}", var, value);
            } else {
                let old_value = replaced_value.unwrap().to_owned();
                debug!("[DefaultAssign] replacing variable {} = {} (old value = {})",
                    var, value, old_value);
            }
        }
        user_context.set_default_value(var, value);
        Ok(())
    }
}

impl Command for EchoCommand {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        Ok(command.get(0) == "echo")
    }

    fn execute(&self,
               tokens: &Tokens,
               _user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        return match Self::_write_all(&tokens, io_context) {
            Ok(_) => Ok(()),
            Err(e) => Err(ShellError::IoError {
                src: io_context.to_source_info(),
                tokens: tokens.clone(),
                error: e,
            })
        }
    }
}

impl EchoCommand {
    fn _write_all(command: &&Tokens, io_context: &mut IoContext) -> Result<(), io::Error> {
        for i in 1..command.len() {
            if i != 1 {
                io_context.write_str(" ")?;
            }
            io_context.write_str(&command.get(i))?;
        }
        Ok(())
    }
}

impl Command for ExecuteCommand {
    fn name(&self) -> &'static str {
        "execute"
    }

    fn validate(&self, _command: &Tokens) -> Result<bool, CommandValidationError> {
        Ok(true)
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        let mut args: Vec<&str> = vec![];
        for i in 1..tokens.len() {
            args.push(&tokens.get(i));
        }

        debug!("[Execute] invoking method pwd={}, cd={}, args={}",
            user_context.pwd(), &tokens.get(0), args.join(", "));
        let result = shell.registry.parsed_invoke_method(user_context.pwd(),
                                                         &tokens.get(0),
                                                         &args)
            .map_err(|e| ShellError::RegistryError {
                src: io_context.to_source_info(),
                tokens: tokens.clone(),
                error: e,
            })?;

        write_object(io_context, shell, &result).map_err(|e| ShellError::IoError {
            src: io_context.to_source_info(),
            tokens: tokens.clone(),
            error: e,
        })?;
        Ok(())
    }
}

impl Command for LsCommand {
    fn name(&self) -> &'static str {
        "ls"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        if command.get(0) == "ls" {
            return if command.len() == 1 || command.len() == 2 {
                Ok(true)
            } else {
                Err(CommandValidationError::InvalidCommandFormat {
                    format: "ls [dir]"
                })
            }
        }
        Ok(false)
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        let registry = &shell.registry;
        let mut children: Vec<String> = vec![];

        let cd = if tokens.len() == 1 { "." } else { &tokens.get(1) };

        let path = registry.cd(&user_context.pwd, cd)
            .map_err(|e| ShellError::RegistryError {
                src: io_context.to_source_info(),
                tokens: tokens.clone(),
                error: e,
            })?;

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
            let e = io_context.write_string(child).err();
            if e.is_some() {
                return Err(ShellError::IoError {
                    src: io_context.to_source_info(),
                    tokens: tokens.clone(),
                    error: e.unwrap(),
                })
            }
        }

        Ok(())
    }
}

fn write_object(io_context: &mut IoContext, shell: &Shell, result: &PolarValue)
                -> Result<(), io::Error> {
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
    fn name(&self) -> &'static str {
        "mkdir"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        if command.get(0) == "mkdir" {
            return if command.len() == 2 {
                Ok(true)
            } else {
                Err(CommandValidationError::InvalidCommandFormat {
                    format: "mkdir <dir>"
                })
            };
        }
        Ok(false)
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        debug!("[MkDir] creating directories pwd={}, cd={}",
            user_context.pwd(), tokens.get(1));
        shell.registry.mkdir(user_context.pwd(), tokens.get(1)).map_err(
            |e| ShellError::RegistryError {
                src: io_context.to_source_info(),
                tokens: tokens.clone(),
                error: e,
            })
    }
}

impl Command for PwdCommand {
    fn name(&self) -> &'static str {
        "pwd"
    }

    fn validate(&self, tokens: &Tokens) -> Result<bool, CommandValidationError> {
        if tokens.get(0) == "pwd" {
            return if tokens.len() == 1 {
                Ok(true)
            } else {
                Err(CommandValidationError::InvalidCommandFormat {
                    format: "pwd",
                })
            }
        }
        Ok(false)
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        io_context.write_str(&user_context.pwd).map_err(|e| ShellError::IoError {
            src: io_context.to_source_info(),
            tokens: tokens.clone(),
            error: e,
        })
    }
}

const MAX_SOURCE_RECURSION: usize = 10;

impl Command for SourceCommand {
    fn name(&self) -> &'static str {
        "source"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        let len = command.len();
        if command.get(0) == "source" {
            return if len == 2 && command.get(1) != "-s" || len >= 3 {
                Ok(true)
            } else {
                Err(CommandValidationError::InvalidCommandFormat {
                    format: "source [-s] <file>"
                })
            };
        }
        Ok(false)
    }

    fn execute(&self,
               tokens: &Tokens,
               user_context: &mut UserContext,
               io_context: &mut IoContext,
               _command_context: &CommandContext,
               shell: &mut Shell) -> Result<(), ShellError> {
        let subshell = tokens.get(1) == "-s";
        let arg_start = if subshell { 3 } else { 2 };
        let file_name = tokens.get(arg_start - 1);

        debug!("[Source] loading file {}{}, args={}",
            file_name,
            if subshell { " (subshell)" } else { "" },
            tokens.tokens_substring(arg_start, tokens.len()));

        // make a copy of all variables
        let mut new_user_context = user_context.clone();
        // add our arguments to the file
        new_user_context.clear_arguments();
        for i in arg_start..tokens.len() {
            let arg = tokens.get(i);
            new_user_context.add_argument(arg);
        }

        // load file
        match File::open(file_name) {
            Ok(f) => {
                if user_context.level >= MAX_SOURCE_RECURSION {
                    return Err(ShellError::CommandExecutionError {
                        src: io_context.to_source_info(),
                        tokens: tokens.clone(),
                        error: CommandExecutionError::MaxSourceCommand(user_context.level)
                    })
                }

                let mut reader = BufReader::new(f);
                // the new I/O context has the new file, but the same output
                let mut new_io_context = IoContext::new(
                    file_name, &mut reader, &mut io_context.output);

                user_context.level += 1;
                shell.execute_commands(
                    &mut new_user_context, &mut new_io_context, _command_context)?;
                user_context.level -= 1;
            }
            Err(error) => return Err(ShellError::CommandExecutionError {
                src: io_context.to_source_info(),
                tokens: tokens.clone(),
                error: CommandExecutionError::UnableToOpenFile {
                    file: file_name.to_owned(),
                    error,
                }
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
    fn name(&self) -> &'static str {
        "unset"
    }

    fn validate(&self, command: &Tokens) -> Result<bool, CommandValidationError> {
        if command.get(0) == "unset" {
            let len = command.len();
            if len == 1 {
                return Err(CommandValidationError::InvalidCommandFormat {
                    format: "unset [var ...]",
                });
            }
            for i in 1..len {
                if !validate_variable(&command.get(i)) {
                    return Err(CommandValidationError::InvalidVariableName(
                        command.get(i).to_owned()));
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn execute(&self,
               command: &Tokens,
               user_context: &mut UserContext,
               _io_context: &mut IoContext,
               _command_context: &CommandContext,
               _shell: &mut Shell) -> Result<(), ShellError> {
        debug!("[Unset] removing variables {}",
            command.tokens_substring(1, command.len()));
        for i in 1..command.len() {
            user_context.remove_value(&command.get(i));
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

fn validate_assigment(tokens: &Tokens, sign: &'static str)
                      -> Result<bool, CommandValidationError> {

    if tokens.len() == 3 && tokens.get(1) == sign {
        if validate_variable(tokens.get(0)) {
            Ok(true)
        } else {
            Err(CommandValidationError::InvalidVariableName(tokens.get(0).to_owned()))
        }
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod assign_tests {
    use std::io;
    use crate::command::commands::{AssignCommand, Command, CommandValidationError};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let command = AssignCommand {};

        let result = command.validate(&Tokens::new(
            vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()])).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let command = AssignCommand {};

        let result = command.validate(&Tokens::new(
            vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()])).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let command = AssignCommand {};
        let tokens = Tokens::new(vec!["12foo".to_owned(), "=".to_owned(), "bar".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidVariableName("12foo".to_owned()), result);
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
        let tokens = Tokens::new(vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()]);
        command.validate(&tokens).unwrap();

        command.execute(&tokens, &mut context, &mut source, &command_context, &mut shell).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
    }
}

#[cfg(test)]
mod default_assign_tests {
    use std::io;
    use crate::command::commands::{DefaultAssignCommand, Command, CommandValidationError};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;

    #[test]
    fn validate_valid_assignment_command_returns_true() {
        let command = DefaultAssignCommand {};

        let result = command.validate(&Tokens::new(
            vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()])).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_invalid_assignment_command_returns_false() {
        let command = DefaultAssignCommand {};

        let result = command.validate(&Tokens::new(
            vec!["foo".to_owned(), "=".to_owned(), "bar".to_owned()])).unwrap();

        assert!(!result);
    }

    #[test]
    fn validate_invalid_variable_name_returns_error() {
        let command = DefaultAssignCommand {};
        let tokens = Tokens::new(vec!["12foo".to_owned(), ":=".to_owned(), "bar".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidVariableName("12foo".to_owned()), result);
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
        let tokens = Tokens::new(vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()]);
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
        let tokens = Tokens::new(vec!["foo".to_owned(), ":=".to_owned(), "bar".to_owned()]);
        command.validate(&tokens).unwrap();

        command.execute(&tokens, &mut context, &mut source, &command_context, &mut shell).unwrap();

        assert_eq!("soo", context.get_value("foo").unwrap());
    }
}

#[cfg(test)]
mod unset_tests {
    use std::io;
    use crate::command::commands::{Command, CommandValidationError, UnsetCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;

    #[test]
    fn validate_valid_unset_command_with_one_variable_returns_true() {
        let command = UnsetCommand {};

        let result = command.validate(&Tokens::new(vec!["unset".to_owned(), "foo".to_owned()])).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_valid_unset_command_with_multiple_variables_returns_true() {
        let command = UnsetCommand {};

        let result = command.validate(&Tokens::new(
            vec!["unset".to_owned(), "foo".to_owned(), "bar".to_owned()])).unwrap();

        assert!(result);
    }

    #[test]
    fn different_command_returns_false() {
        let command = UnsetCommand {};

        let result = command.validate(&Tokens::new(
            vec!["foo".to_owned(), "=".to_owned(), "unset".to_owned()])).unwrap();

        assert!(!result);
    }

    #[test]
    fn unset_with_no_variables_is_error() {
        let command = UnsetCommand {};
        let tokens = Tokens::new(vec!["unset".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidCommandFormat {
            format: "unset [var ...]",
        }, result);
    }

    #[test]
    fn unset_invalid_variable_name_returns_error() {
        let command = UnsetCommand {};
        let tokens = Tokens::new(vec!["unset".to_owned(), "foo".to_owned(), "12foo".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidVariableName("12foo".to_owned()), result);
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
        let tokens = Tokens::new(vec!["unset".to_owned(), "soo".to_owned(), "goo".to_owned()]);

        command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        assert_eq!("bar", context.get_value("foo").unwrap());
        assert!(context.get_value("soo").is_none());
        assert!(context.get_value("goo").is_none());
    }
}

#[cfg(test)]
mod mkdir_tests {
    use std::io;
    use crate::command::commands::{Command, CommandValidationError, MkDirCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;

    #[test]
    fn validate_valid_mkdir_command_returns_true() {
        let command = MkDirCommand {};

        let result = command.validate(&Tokens::new(vec!["mkdir".to_owned(), "foo".to_owned()])).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_mkdir_command_without_arg_returns_error() {
        let command = MkDirCommand {};
        let tokens = Tokens::new(vec!["mkdir".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidCommandFormat { format: "mkdir <dir>" }, result);
    }

    #[test]
    fn validate_mkdir_command_with_multiple_args_returns_error() {
        let command = MkDirCommand {};
        let tokens = Tokens::new(vec!["mkdir".to_owned(), "foo".to_owned(), "bar".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidCommandFormat { format: "mkdir <dir>" }, result);
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
        let tokens = Tokens::new(vec!["mkdir".to_owned(), "foo/bar".to_owned()]);

        command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        let path = shell.registry.path("/foo/bar").unwrap();
        assert_eq!("/foo/bar", path.abs_path());
    }
}

#[cfg(test)]
mod cd_tests {
    use std::io;
    use crate::command::commands::{Command, CdCommand, MkDirCommand, CommandValidationError};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;

    #[test]
    fn validate_valid_cd_command_returns_error() {
        let command = CdCommand {};
        let tokens = Tokens::new(vec!["cd".to_owned(), "foo".to_owned(), "bar".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidCommandFormat { format: "cd <dir>" }, result);
    }

    #[test]
    fn validate_cd_command_without_arg_returns_error() {
        let command = CdCommand {};
        let tokens = Tokens::new(vec!["cd".to_owned()]);

        let result = command.validate(&tokens).err().unwrap();

        assert_eq!(CommandValidationError::InvalidCommandFormat { format: "cd <dir>" }, result);
    }

    #[test]
    fn validate_cd_command_with_one_arg_returns_true() {
        let command = CdCommand {};
        let tokens = Tokens::new(vec!["cd".to_owned(), "foo/bar".to_owned()]);

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
            &Tokens::new(vec!["mkdir".to_owned(), "foo/bar/soo".to_owned()]),
            &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        CdCommand {}.execute(
            &Tokens::new(vec!["cd".to_owned(), "foo/bar".to_owned()]),
            &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        assert_eq!("/foo/bar", context.pwd);
    }
}

#[cfg(test)]
mod echo_tests {
    use std::io;
    use std::io::Cursor;
    use crate::command::commands::{Command, EchoCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;

    #[test]
    fn validate_valid_echo_command_returns_true() {
        let command = EchoCommand {};

        let result = command.validate(&Tokens::new(
            vec!["echo".to_owned(), "foo".to_owned(), "bar".to_owned()])).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_echo_command_without_args_returns_true() {
        let command = EchoCommand {};

        let result = command.validate(&Tokens::new(vec!["echo".to_owned()])).unwrap();

        assert!(result);
    }

    #[test]
    fn non_echo_command_returns_false() {
        let command = EchoCommand {};

        let result = command.validate(
            &Tokens::new(vec!["mkdir".to_owned(), "foo".to_owned()])).unwrap();

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
        let tokens = Tokens::new(vec!["echo".to_owned(), "foo".to_owned(), "me".to_owned()]);

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
    use crate::command::commands::{Command, CommandValidationError, CreateCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;
    use crate::command::oso::PolarClass;
    use crate::command::{RegistryError, ShellError};

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

        let result = command.validate(&Tokens::new(vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
                "core::command::commands::create_tests::User".to_owned(),
                "jgreco".to_owned(),
                "42".to_owned()
            ])).unwrap();

        assert!(result);
    }

    #[test]
    fn validate_create_command_with_1_param_is_error() {
        let command = CreateCommand {};
        let token = Tokens::new(vec![
                "create".to_owned()
            ]);

        let result = command.validate(&token).err().unwrap();

        assert_eq!(result, CommandValidationError::InvalidCommandFormat {
            format: "create <dir> <struct> [args ...]"
        });
    }

    #[test]
    fn validate_create_command_with_2_params_returns_false() {
        let command = CreateCommand {};
        let token = Tokens::new(vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
            ]);

        let result = command.validate(&token).err().unwrap();

        assert_eq!(result, CommandValidationError::InvalidCommandFormat {
            format: "create <dir> <struct> [args ...]"
        });
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
        let tokens = Tokens::new(vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
                "rcore::command::commands::create_tests::User".to_owned(),
                "jgreco".to_owned(),
                "42".to_owned()
            ]);

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
        let tokens = Tokens::new(vec![
                "create".to_owned(),
                "/foo/bar".to_owned(),
                "rcore::command::commands::create_tests::User".to_owned(),
                "jgreco".to_owned(),
                "42".to_owned()
            ]);

        let result = command.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).err().unwrap();

        assert_eq!(result, ShellError::RegistryError {
            src: io_context.to_source_info(),
            tokens,
            error: RegistryError::InvalidMethodParameter {
                class: "rcore::command::commands::create_tests::User".to_owned(),
                method: "<constructor>".to_owned(),
                param_index: 0,
                param_type: "int",
                reason: "",
            }
        })
    }
}

#[cfg(test)]
mod execute_tests {
    use std::io;
    use std::io::Cursor;
    use crate::command::commands::{Command, CreateCommand, ExecuteCommand};
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::Tokens;
    use crate::command::shell::Shell;
    use crate::command::oso::PolarClass;
    use crate::command::{RegistryError, ShellError};

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
            &Tokens::new(vec![
                    "create".to_owned(),
                    "/foo/bar".to_owned(),
                    "rcore::command::commands::execute_tests::User".to_owned(),
                    "jgreco".to_owned(),
                    "42".to_owned()
                ]), &mut context, &mut io_context, &command_context, &mut shell).unwrap();

        ExecuteCommand {}.execute(
            &Tokens::new(vec![
                    "/foo/bar/add".to_owned(),
                    "41".to_owned()
                ]), &mut context, &mut io_context, &command_context, &mut shell).unwrap();

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
            &Tokens::new(vec![
                    "create".to_owned(),
                    "/foo/bar".to_owned(),
                    "rcore::command::commands::execute_tests::User".to_owned(),
                    "jgreco".to_owned(),
                    "42".to_owned()
                ]), &mut context, &mut io_context, &command_context, &mut shell).unwrap();
        let tokens = Tokens::new(vec![
                "/foo/bar/add".to_owned(),
                "foo".to_owned()
            ]);

        let err = ExecuteCommand {}.execute(
            &tokens, &mut context, &mut io_context, &command_context, &mut shell).err().unwrap();

        assert_eq!(ShellError::RegistryError {
            src: io_context.to_source_info(),
            error: RegistryError::InvalidMethodParameter {
                class: "User".to_owned(),
                method: "add_one".to_string(),
                param_index: 0,
                param_type: "int",
                reason: "",
            },
            tokens: tokens.clone(),
        }, err);
    }
}