use std::collections::HashMap;
use std::io::{Cursor, Read, stdout, Write};
use std::io;
use crate::command::lexer::TokenGroup;

#[derive(Default, Clone)]
pub struct Context {
    pwd: String,
    variables: HashMap<String, String>,
    arguments: Vec<String>
}

impl Context {
    pub(crate) fn update_pwd(&mut self, pwd: &str) {
        self.pwd.clear();
        self.pwd.push_str(pwd);
    }

    pub(crate) fn get_argument(&self, position: usize) -> Option<&String> {
        self.arguments.get(position)
    }

    pub(crate) fn add_argument(&mut self, value: &str) {
        self.arguments.push(value.to_owned());
    }

    pub(crate) fn clear_arguments(&mut self) {
        self.arguments.clear();
    }

    pub(crate) fn get_value(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    pub(crate) fn set_value(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_owned(), value.to_owned());
    }

    pub(crate) fn set_default_value(&mut self, key: &str, value: &str) {
        if !self.variables.contains_key(key) {
            self.variables.insert(key.to_owned(), value.to_owned());
        }
    }
}

pub(crate) struct Source<'a> {
    pub(crate) source: &'a str,
    pub(crate) line: usize,
    pub(crate) column: usize,
    input: &'a mut dyn Read,
    output: &'a mut dyn Write,
    buffer: [u8; 1]
}

impl<'a> Source<'a> {
    pub(crate) fn new(name: &'a str, input: &'a mut dyn Read, output: &'a mut dyn Write) -> Self {
        Source {
            source: name,
            line: 0,
            column: 0,
            input,
            output,
            buffer: [0]
        }
    }

    pub(crate) fn new_test(input: &'a mut dyn Read, output: &'a mut dyn Write) -> Self {
        Self::new("test", input, output)
    }

    pub(crate) fn cursor(text: &'static str) -> Box<dyn Read> {
        Box::new(Cursor::new(text.as_bytes()))
    }

    pub(crate) fn stdout() -> Box<dyn Write> {
        Box::new( stdout())
    }

    pub(crate) fn next_byte(&mut self) -> Result<Option<u8>, io::Error> {
        // TODO: extend to support unicode
        let bytes_read = self.input.read(&mut self.buffer)?;
        if bytes_read == 1 {
            Ok(Some(self.buffer[0]))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn write_str(&mut self, string: &str) -> Result<(), io::Error> {
        self.output.write_all(string.as_bytes())
    }
}
