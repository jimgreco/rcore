use std::io;
use log::{Level, debug};
use crate::command::context::{UserContext, IoContext, CommandContext};
use crate::command::lexer::{lex_command, LexerError, TokenGroup};

use thiserror::Error;
use crate::command::{Registry, RegistryError};
use crate::command::oso::Class;

#[derive(Default)]
pub struct Shell {
    pub(crate) registry: Registry,
}

impl Shell {
    pub fn cache_class(&mut self, class: Class) -> Result<(), RegistryError> {
        self.registry.cache_class(class)
    }

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

                        for command in &command_context.commands {
                            if command.validate(&token_group)? {
                                command.execute(
                                    &token_group, user_context, io_context, command_context, self)?;
                                executed = true;
                                break;
                            }
                        }

                        if !executed {
                            return Err(ShellError::UnknownCommand(token_group));
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
        (Shell::default(), CommandContext::new(), UserContext::new())
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