use std::io::{Read, Write};
use crate::command::context::{Context, Source};
use crate::command::lexer::LexerError;
use crate::command::parser::{parse_command, ParserError};

use thiserror::Error;

/// Errors thrown executing commands in the command file.
#[derive(Debug, PartialEq, Error)]
pub enum ShellError {
    #[error(transparent)]
    LexerError(LexerError),
    #[error(transparent)]
    ParserError(ParserError),
}

fn execute_commands(
        source: &str,
        input: &mut dyn Read,
        output: &mut dyn Write,
        context: &mut Context) -> Result<(), ShellError> {
    let mut source = Source::new(source, input, output);

    loop {
        match parse_command(&context, &mut source) {
            Some(result) => match result {
                Ok(command) => {
                    let result = command.execute(context)?;
                    if result.is_some() {
                        let x = result.unwrap();
                    }
                }
                Err(e) => {
                    return Err(match e {
                        ParserError::LexerError(e) => ShellError::LexerError(e),
                        e => ShellError::ParserError(e)
                    });
                }
            }
            None => return Ok(())
        }
    }
}
