use std::io;
use std::io::{Read, Write};
use log::warn;
use crate::command::context::{Context, Source};
use crate::command::lexer::LexerError;
use crate::command::parser::{parse_command, ParserError};

use thiserror::Error;
use crate::command::commands::Command;

/// Errors thrown executing commands in the command file.
#[derive(Debug, Error)]
pub enum ShellError {
    #[error(transparent)]
    LexerError(LexerError),
    #[error(transparent)]
    ParserError(ParserError),
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
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

pub struct Shell {
    command_specs: Vec<Box<dyn Command>>
}

impl Shell {
    pub fn add_command_spec(&mut self, spec: Box<dyn Command>) {
        self.command_specs.push(spec);
    }

    fn execute_commands(
            &self,
            source: &str,
            input: &mut dyn Read,
            output: &mut dyn Write,
            context: &mut Context) -> Result<(), ShellError> {
        let mut source = Source::new(source, input, output);

        loop {
            match parse_command(&context, &mut source, &self.command_specs) {
                Ok(result) => match result {
                    Some((command, spec)) => {
                        let option = spec.execute(&command, context)?;
                        if option.is_some() {
                            let x = option.unwrap();
                        }
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
