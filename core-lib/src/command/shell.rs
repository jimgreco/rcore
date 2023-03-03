use std::io;
use log::{Level, debug};
use crate::command::context::{UserContext, IoContext, CommandContext};
use crate::command::lexer::{lex_command, LexerError, TokenGroup};

use thiserror::Error;
use crate::command::{Registry, RegistryError};
use crate::command::oso::Class;

/// The command shell is used to dynamically instantiate instances of structs, invoke methods on
/// instances, and get the attribute values from instances at startup or run-time.
/// Similar to the Unix shell, instances, methods, and attributes are stored in a directory
/// structure that can be accessed or invoked through text-based commands.
///
/// # Variables
/// The shell supports variables for use in commands.
///
/// - `<var> = <value>`: assigns a value to a variable
/// - `<var> := <value>`: assigns a value to a variable if it does not yet exist
/// - `unset <var> [var ...]`: removes a variable from the shell
/// - `echo [arg ...]`: writes back the provided arguments
///
/// Variables can only contain alphanumeric or underscore characters and must start with an
/// alphabetic or underscore character.
/// Variables can be accessed in other commands with the `$` sign.
///
/// ```
/// let (result, _) = rcore::command::Shell::from_string(
///     "v1 = abc                # v1 = abc
///      v1 := def               # v1 = abc (already has value, not overridden)
///      v2 := hij               # v2 = hij
///      v2 = klm                # v2 = klm (overridden)
///      v3 := \"nop   qrs\"     # v3 = nop, quotes allow for spaces in values
///      echo $v1 $v2 $v3").unwrap();
///
/// assert_eq!("abc klm nop   qrs", &result);
/// ```
///
/// # Built-in Commands
/// The following commands are built into the shell to facilitate the creation and navigation of the
/// directory structure, echoing back arguments, and loading command files.
///
/// - `cd <dir>`: changes the current working directory of the user
/// - `ls [dir]`: lists the contents of the current user directory
/// - `pwd`: the current working directory of the user
/// - `mkdir <dir>`: creates a new directory
///
/// ```
/// let (result, user_context) = rcore::command::Shell::from_string(
///     "mkdir /foo/bar    # pwd = /
///      cd foo/bar        # pwd = /foo/bar
///      mkdir me
///      cd me             # pwd = /foo/bar/me
///      cd ../..          # pwd = /foo
///      pwd").unwrap();
///
/// assert_eq!("/foo", &user_context.pwd);
/// assert_eq!("/foo", &result);
/// ```
///
/// # Loading Commands Files
/// The commands provided to the Shell can take any format that implements the [io::Read] interface.
/// Commands files can be loaded from the filesystem with the `source` command.
///
/// - `source [-s] <file> [arg ...]`: loads a commands file. The `-s`
///
/// ```
/// use std::fs::File;
/// use std::io::Write;
///
/// let mut file = File::create("/tmp/from_a_file1.commands").unwrap();
/// file.write_all(
///     "mkdir /foo/bar/me
///      cd foo/bar".as_bytes()).unwrap();
/// let mut file = File::create("/tmp/from_a_file2.commands").unwrap();
/// file.write_all(
///     "mkdir /do/re/me
///      cd /do/re".as_bytes()).unwrap();
/// let (result, user_context) = rcore::command::Shell::from_string(
///     "source /tmp/from_a_file1.commands
///      source -s /tmp/from_a_file2.commands  # sub-shell does not affect user state
///      cd me
///      pwd").unwrap();
///
/// assert_eq!("/foo/bar/me", &user_context.pwd);
/// assert_eq!("/foo/bar/me", &result);
/// ```
///
/// # Instance Commands
/// The primary usage of the command shell is to create instances of structs and invoke methods on
/// or retrieve the value of attributes from those instances.
///
/// - `create <dir> <struct_name> [arg ...]`: instantiates an instance of a struct
/// - `</path/to/method_or_attribute> [arg ...]`: invokes a method or retrieves the value of an
///    attribute
///
/// The user can configure the [CommandContext] with user-defined commands.
#[derive(Default)]
pub struct Shell {
    /// The command registry.
    pub registry: Registry,
}

impl Shell {
    /// Caches a [Class] which describes a struct, a function to create instances ("constructor"),
    /// getters for the instance's attributes, and its instance functions ("instance methods").
    pub fn cache_class(&mut self, class: Class) -> Result<(), RegistryError> {
        self.registry.cache_class(class)
    }

    /// Executes one or more commands through the shell.
    /// The [UserContext] holds the user's variables and current working directory.
    /// The [IoContext] specifies the input commands to execute and the output to the results to.
    /// The [CommandContext] specifies the universe of commands that can be executed.
    ///
    /// # Example
    /// ```
    /// use rcore::command::{CommandContext, IoContext, Shell, UserContext};
    /// let mut shell = Shell::default();
    /// let mut input = std::io::Cursor::new(
    ///         "v1 = \"Mr. Burns\"
    ///          v2 = 42
    ///          echo $v1 $v2".as_bytes());
    /// let mut output_vec: Vec<u8> = Vec::new();
    /// let mut output = std::io::Cursor::new(&mut output_vec);
    /// let mut io_context = IoContext::new("test", &mut input, &mut output);
    /// let mut user_context = UserContext::default();
    /// let command_context = CommandContext::default();
    /// let mut shell = Shell::default();
    ///
    /// shell.execute_commands(&mut user_context, &mut io_context, &command_context).unwrap();
    ///
    /// assert_eq!("Mr. Burns 42", &String::from_utf8(output_vec).unwrap());
    /// ```
    pub fn execute_commands(&mut self,
                            user_context: &mut UserContext,
                            io_context: &mut IoContext,
                            command_context: &CommandContext) -> Result<(), ShellError> {
        loop {
            let line = io_context.line;
            match lex_command(user_context, io_context) {
                Some(result) => match result {
                    Ok(token_group) => {
                        if log::log_enabled!(Level::Debug) {
                            debug!("{}:{}: {}",
                                io_context.source,
                                line,
                                token_group.tokens_string());
                        }

                        let mut executed = false;

                        for command in &command_context.builtin_commands {
                            if command.validate(&token_group)? {
                                command.execute(
                                    &token_group, user_context, io_context, command_context, self)?;
                                executed = true;
                                break;
                            }
                        }

                        if !executed {
                            command_context.execute_command.execute(
                                &token_group, user_context, io_context, command_context, self)?;
                        }
                    }
                    Err(e) => return Err(match e {
                        LexerError::IoError { error, .. } => ShellError::IoError(error),
                        e => ShellError::LexerError(e)
                    })
                }
                None => return Ok(())
            }
        }
    }

    /// This is a utility method to run commands from a string.
    /// This method is primarily designed to simplify the running of commands in documentation and
    /// tests and should not be used in production.
    ///
    /// # Example
    /// ```
    /// let (result, _) = rcore::command::Shell::from_string(
    ///         "v1 = \"Mr. Burns\"
    ///          v2 = 42
    ///          echo $v1 $v2").unwrap();
    ///
    /// assert_eq!("Mr. Burns 42", result);
    /// ```
    pub fn from_string(commands: &str) -> Result<(String, UserContext), ShellError> {
        let mut shell = Shell::default();
        let command_context = CommandContext::default();
        let mut user_context = UserContext::default();
        let mut commands = io::Cursor::new(commands.as_bytes());
        let mut output_vec: Vec<u8> = Vec::new();
        let mut output = io::Cursor::new(&mut output_vec);
        let mut io_context = IoContext::new("test", &mut commands, &mut output);

        shell.execute_commands(&mut user_context, &mut io_context, &command_context)?;

        Ok((String::from_utf8(output_vec).unwrap(), user_context))
    }
}

/// Errors thrown executing commands by the shell.
#[derive(Debug, Error)]
pub enum ShellError {
    #[error(transparent)]
    LexerError(LexerError),
    #[error("Error executing command on the registry: {command}, error={error}")]
    RegistryCommandError {
        command: TokenGroup,
        error: RegistryError,
    },
    #[error("Error accessing the registry: {0}")]
    RegistryError(RegistryError),
    #[error("File does not exist: {file}, error={error}")]
    UnknownFile {
        file: String,
        error: io::Error,
    },
    #[error("invalid variable name: {var}, command={command}")]
    InvalidVariableName {
        command: TokenGroup,
        var: String,
    },
    #[error("invalid formatted command: {0}")]
    InvalidCommandFormat(TokenGroup),
    #[error("unknown command: {0}")]
    UnknownCommand(TokenGroup),
    #[error("I/O error: {0}")]
    IoError(io::Error),
}

impl PartialEq for ShellError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ShellError::LexerError(e1), ShellError::LexerError(e2))
            => e1 == e2,
            (ShellError::RegistryCommandError { command, error },
                ShellError::RegistryCommandError { command: command2, error: error2 })
            => command == command2 && error == error2,
            (ShellError::UnknownFile { file, .. }, ShellError::UnknownFile { file: file2, .. })
            => file == file2,
            (ShellError::InvalidVariableName { command, var },
                ShellError::InvalidVariableName { command: command2, var: var2 })
            => command.eq(command2) && var.eq(var2),
            (ShellError::InvalidCommandFormat(e1), ShellError::InvalidCommandFormat(e2))
            => e1.eq(e2),
            (ShellError::UnknownCommand(e1), ShellError::UnknownCommand(e2))
            => e1.eq(e2),
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::Cursor;
    use crate::command::context::{UserContext, IoContext, CommandContext};
    use crate::command::lexer::{LexerError, TokenGroup};
    use crate::command::shell::{Shell, ShellError};

    fn setup() -> (Shell, CommandContext, UserContext) {
        (Shell::default(), CommandContext::default(), UserContext::default())
    }

    #[test]
    fn execute_one_commands() {
        let (mut shell, commands, mut user_context) = setup();
        let mut cursor = Cursor::new("foo = bar".as_bytes());
        let mut sink = io::sink();
        let mut io_context = IoContext::new("test", &mut cursor, &mut sink);

        let result = shell.execute_commands(&mut user_context, &mut io_context, &commands).unwrap();

        assert_eq!((), result);
        assert_eq!("bar", user_context.get_value("foo").unwrap());
    }

    #[test]
    fn execute_multiple_commands() {
        let (mut shell, commands, mut user_context) = setup();
        let mut cursor = Cursor::new("foo = bar\nfoo := soo\ndo12 = goo".as_bytes());
        let mut sink = io::sink();
        let mut io_context = IoContext::new("test", &mut cursor, &mut sink);

        let result = shell.execute_commands(&mut user_context, &mut io_context, &commands).unwrap();

        assert_eq!((), result);
        assert_eq!("bar", user_context.get_value("foo").unwrap());
        assert_eq!("goo", user_context.get_value("do12").unwrap());
    }

    #[test]
    fn lexer_error_is_passed_through() {
        let (mut shell, commands, mut user_context) = setup();
        let mut cursor = Cursor::new("foo = bar
foo = s\"oo
do12 = goo".as_bytes());
        let mut sink = io::sink();
        let mut io_context = IoContext::new("test", &mut cursor, &mut sink);

        let result = shell.execute_commands(&mut user_context, &mut io_context, &commands).err().unwrap();

        assert_eq!(ShellError::LexerError(LexerError::UnterminatedQuote {
            src: "test".to_owned(),
            line: 2,
            col: 7,
        }), result);
    }

    #[test]
    fn invalid_command_throws_error() {
        let (mut shell, commands, mut user_context) = setup();
        let mut cursor = Cursor::new("foo = bar
                    12foo = soo
                    do12 = goo".as_bytes());
        let mut sink = io::sink();
        let mut io_context = IoContext::new("test", &mut cursor, &mut sink);

        let result = shell.execute_commands(&mut user_context, &mut io_context, &commands).err().unwrap();

        assert_eq!(ShellError::InvalidVariableName {
            command: TokenGroup {
                line: 2,
                tokens: vec!["12foo".to_owned(), "=".to_owned(), "soo".to_owned()],
            },
            var: "12foo".to_string(),
        }, result);
    }
}