use std::io;
use crate::command::context::{UserContext, IoContext};
use crate::command::lexer::LexerError;
use crate::command::parser::{parse_command, ParserError};

use thiserror::Error;
use crate::command::commands::{AssignCommand, Command, DefaultAssignCommand, SourceCommand, UnsetCommand};
use crate::command::Registry;

/// Errors thrown executing commands in the command file.
#[derive(Debug, Error)]
pub enum ShellError {
    #[error(transparent)]
    LexerError(LexerError),
    #[error(transparent)]
    ParserError(ParserError),
    #[error("File does not exist: {file}, error={error}")]
    UnknownFile {
        file: String,
        error: io::Error
    },
    #[error("I/O error: {0}")]
    Io(io::Error)
}

impl PartialEq for ShellError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ShellError::LexerError(e1), ShellError::LexerError(e2))
                => e1.eq(e2),
            (ShellError::ParserError(e1), ShellError::ParserError(e2))
                => e1.eq(e2),
            (ShellError::UnknownFile { file, ..}, ShellError::UnknownFile { file: file2, ..})
                => file.eq(file2),
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

pub struct Shell {
    commands: Vec<Box<dyn Command>>,
    pub(crate) registry: Registry
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            commands: vec![Box::new(AssignCommand {}),
                           Box::new(DefaultAssignCommand {}),
                           Box::new(UnsetCommand {}),
                           Box::new(SourceCommand {})],
            registry: Registry::new()
        }
    }

    pub fn add_command(&mut self, spec: Box<dyn Command>) {
        self.commands.push(spec);
    }

    pub fn execute_commands(&self, user_context: &mut UserContext, io_context: &mut IoContext)
                            -> Result<(), ShellError> {
        loop {
            match parse_command(&user_context, io_context, &self.commands) {
                Ok(result) => match result {
                    Some((command, spec)) => {
                        spec.execute(&command, user_context, io_context, &self)?;
                    }
                    None => return Ok(())
                }
                Err(e) => return Err(match e {
                    ParserError::LexerError(e) => ShellError::LexerError(e),
                    ParserError::Io(e) => ShellError::Io(e),
                    e => ShellError::ParserError(e)
                })
            }
        }
    }
}
