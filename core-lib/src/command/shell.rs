use std::collections::HashMap;
use crate::command::lexer::{LexerContext, LexerError};
use crate::command::parser::{Parser, ParserError};

pub enum ShellError {
    LexerError(LexerError),
    ParserError(ParserError),
}

#[derive(Default)]
pub struct ShellContext {
    pwd: String,
    variables: HashMap<String, String>,
    arguments: Vec<String>,
}

impl ShellContext {
    fn new() -> ShellContext {
        ShellContext {
            pwd: "/".to_owned(),
            variables: Default::default(),
            arguments: vec![],
        }
    }
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
    fn execute(commands_file: &str, context: &mut ShellContext) -> Result<(), ShellError> {
        let mut parser = Parser::new();
        loop {
            let mut text = commands_file.chars();
            match parser.next(&mut text, context) {
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