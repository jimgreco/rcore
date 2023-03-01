use std::collections::HashMap;
use std::io::{Read, Write};
use std::io;
use crate::command::commands::{AssignCommand, Command, DefaultAssignCommand, SourceCommand, UnsetCommand};

#[derive(Default, Clone)]
pub struct UserContext {
    pub pwd: String,
    pub variables: HashMap<String, String>,
    arguments: Vec<String>,
}

impl UserContext {
    pub(crate) fn set_pwd(&mut self, pwd: &str) {
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

    pub(crate) fn remove_value(&mut self, key: &str) {
        self.variables.remove(key);
    }

    pub(crate) fn clear_variables(&mut self) {
        self.variables.clear();
    }
}

pub struct IoContext<'a> {
    pub source: &'a str,
    pub line: usize,
    pub column: usize,
    pub input: &'a mut dyn Read,
    pub output: &'a mut dyn Write,
    buffer: [u8; 1],
}

impl<'a> IoContext<'a> {
    pub fn new(name: &'a str, input: &'a mut dyn Read, output: &'a mut dyn Write) -> Self {
        IoContext {
            source: name,
            line: 0,
            column: 0,
            input,
            output,
            buffer: [0],
        }
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

pub struct CommandContext {
    pub(crate) commands: Vec<Box<dyn Command>>,
}

impl CommandContext {
    pub fn new() -> Self {
        CommandContext {
            commands: vec![Box::new(AssignCommand {}),
                           Box::new(DefaultAssignCommand {}),
                           Box::new(UnsetCommand {}),
                           Box::new(SourceCommand {})],
        }
    }

    pub fn add_command(&mut self, spec: Box<dyn Command>) {
        self.commands.push(spec);
    }
}