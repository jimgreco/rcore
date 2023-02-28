use std::any::Any;
use std::collections::HashMap;
use crate::command::commands::ExecutableCommand;
use crate::command::lexer::{LexerContext, LexerError};
use crate::command::parser::{Parser, ParserError};

pub enum ShellError {
    LexerError(LexerError),
    ParserError(ParserError),
}

#[derive(Default)]
pub struct ShellContext {
    variables: HashMap<String, String>,
    arguments: Vec<String>,
}

#[derive(Default)]
pub struct Shell {
}

impl LexerContext for ShellContext {
    fn get_argument(&self, position: usize) -> Option<&String> {
        self.arguments.get(position)
    }

    fn add_argument(&mut self, value: &str) {
        self.arguments.push(value.to_owned());
    }

    fn get_value(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    fn set_value(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_owned(), value.to_owned());
    }

    fn set_default_value(&mut self, key: &str, value: &str) {
        if !self.variables.contains_key(key) {
            self.variables.insert(key.to_owned(), value.to_owned());
        }
    }
}

impl ShellContext {
    fn clear_arguments(&mut self) {
        self.arguments.clear();
    }
}

impl Shell {
    fn load(commands_file: &str, context: &mut ShellContext) -> Result<(), ShellError> {
        let mut parser = Parser::new(commands_file, context);
        loop {
            match parser.next() {
                Some(res) => {
                    match res {
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
                },
                None => return Ok(())
            }
        }
    }
}