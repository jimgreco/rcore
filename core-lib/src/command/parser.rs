use std::fmt::Debug;
use std::io;
use log;
use log::Level;
use crate::command::commands::Command;
use crate::command::lexer::{lex_command, LexerError, TokenGroup};
use crate::command::context::{Context, Source};

use thiserror::Error;
use log::debug;

/// Errors thrown parsing commands in the command file.
#[derive(Debug, Error)]
pub enum ParserError {
    #[error(transparent)]
    LexerError(LexerError),
    #[error("invalid variable name: {var}, command={command}")]
    InvalidVariableName {
        command: TokenGroup,
        var: String
    },
    #[error("invalid formatted command: {0}")]
    InvalidCommandFormat(TokenGroup),
    #[error("unknown command: {0}")]
    UnknownCommand(TokenGroup),
    #[error("I/O error: {0}")]
    Io(io::Error)
}

impl PartialEq for ParserError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ParserError::LexerError(e1), ParserError::LexerError(e2))
                => e1.eq(e2),
            (ParserError::InvalidVariableName { command, var },
                ParserError::InvalidVariableName { command: command2, var: var2 })
                => command.eq(command2) && var.eq(var2),
            (ParserError::UnknownCommand(e1), ParserError::UnknownCommand(e2))
                => e1.eq(e2),
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

pub(crate) fn parse_command<'a>(
        context: &Context, source: &mut Source, specs: &'a Vec<Box<dyn Command>>)
        -> Result<Option<(TokenGroup, &'a Box<dyn Command>)>, ParserError> {
    let line = source.line;
    match lex_command(context, source) {
        Some(result) => {
            match result {
                Ok(mut token_group) => {
                    if log::log_enabled!(Level::Debug) {
                        debug!("{}:{}: {}", source.source, line, token_group.tokens_string());
                    }

                    for spec in specs {
                        match spec.validate(&token_group) {
                            Ok(result) => if result {
                                return Ok(Some((token_group, spec)));
                            }
                            Err(e) => return Err(e)
                        }
                    }

                    return Err(ParserError::UnknownCommand(token_group))
                }
                Err(e) => match e {
                    LexerError::Io { error, .. } => Err(ParserError::Io(error)),
                    e => Err(ParserError::LexerError(e))
                }
            }
        }
        None => Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use crate::command::commands::{AssignmentCommand, DefaultAssignmentCommand, Command,
                                   validate_variable};
    use crate::command::context::{Context, Source};
    use crate::command::lexer::{LexerError, TokenGroup};
    use crate::command::parser::{parse_command, ParserError};
    use crate::command::shell::ShellError;

    fn specs() -> Vec<Box<dyn Command>> {
        let mut vec: Vec<Box<dyn Command>> = vec![];
        vec.push(Box::new(AssignmentCommand {}));
        vec.push(Box::new(DefaultAssignmentCommand {}));
        vec
    }

    #[test]
    fn command_iteration() {
        let specs = specs();
        let context = Context::default();
        let text = "foo = bar\nfoo := soo\ndo12 = goo";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::stdout();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        parse_command(&context, &mut source, &specs).unwrap().unwrap();
        parse_command(&context, &mut source, &specs).unwrap().unwrap();
        parse_command(&context, &mut source, &specs).unwrap().unwrap();

        assert!(parse_command(&context, &mut source, &specs).unwrap().is_none());
    }

    #[test]
    fn lexer_error_is_passed_through() {
        let context = Context::default();
        let specs = specs();
        let text = "foo = bar
foo = s\"oo
do12 = goo
            ";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::stdout();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        parse_command(&context, &mut source, &specs).unwrap().unwrap();

        assert_eq!(ParserError::LexerError(LexerError::UnterminatedQuote {
            src: "test".to_owned(), line: 2, col: 7,
        }), parse_command(&context, &mut source, &specs).err().unwrap());
    }

    #[test]
    fn unknown_command_throws_error() {
        let context = Context::default();
        let specs = specs();
        let text = "foo = bar
            foo /= soo
            do12 = goo
            ";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::stdout();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        parse_command(&context, &mut source, &specs).unwrap().unwrap();

        assert_eq!(ParserError::UnknownCommand(TokenGroup {
            line: 2,
            tokens: vec!["foo".to_owned(), "/=".to_owned(), "soo".to_owned()],
        }), parse_command(&context, &mut source, &specs).err().unwrap());
    }

    #[test]
    fn invalid_command_throws_error() {
        let context = Context::default();
        let specs = specs();
        let text = "foo = bar
            12foo = soo
            do12 = goo
            ";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::stdout();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        parse_command(&context, &mut source, &specs).unwrap().unwrap();

        assert_eq!(ParserError::InvalidVariableName {
            command: TokenGroup {
                line: 2,
                tokens: vec!["12foo".to_owned(), "=".to_owned(), "soo".to_owned()]
            },
            var: "12foo".to_string(),
        }, parse_command(&context, &mut source, &specs).err().unwrap());
    }

    #[derive(Default)]
    struct RemoveVariableCommandSpec {}

    #[derive(Debug)]
    struct RemoveVariableCommand {
        #[warn(dead_code)]
        var: String
    }
}