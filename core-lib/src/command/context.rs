use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::io;
use crate::command::commands::{AssignCommand, CdCommand, Command, CreateCommand, DefaultAssignCommand, EchoCommand, ExecuteCommand, HelpCommand, LsCommand, MkDirCommand, PwdCommand, SourceCommand, UnsetCommand};

/// The user context contains user-specific information related to executing commands in the
/// shell including the current working directory and variables.
///
/// The initial working directory for the user context is the root directory.
#[derive(Clone)]
pub struct UserContext {
    pwd: String,
    pub(crate) variables: HashMap<String, String>,
    arguments: Vec<String>,
    pub(crate) level: usize
}

impl Default for UserContext {
    fn default() -> Self {
        UserContext {
            pwd: "/".to_owned(),
            variables: HashMap::default(),
            arguments: vec![],
            level: 0
        }
    }
}

impl UserContext {
    pub(crate) fn set_pwd(&mut self, pwd: &str) {
        self.pwd.clear();
        self.pwd.push_str(pwd);
    }

    /// Returns the current working directory.
    pub fn pwd(&self) -> &str {
        &self.pwd
    }

    /// Returns the positional argument for the specified index/
    pub fn get_argument(&self, index: usize) -> Option<&String> {
        self.arguments.get(index)
    }

    /// Adds a positional argument.
    pub fn add_argument(&mut self, value: &str) {
        self.arguments.push(value.to_owned());
    }

    /// Clears all position arguments.
    pub fn clear_arguments(&mut self) {
        self.arguments.clear();
    }

    /// Returns the value for the specified variable.
    pub fn get_value(&self, var: &str) -> Option<&String> {
        self.variables.get(var)
    }

    /// Sets the value of the specified variable.
    pub fn set_value(&mut self, var: &str, value: &str) {
        self.variables.insert(var.to_owned(), value.to_owned());
    }

    /// Sets the value of the specified variable if it is not already set.
    pub fn set_default_value(&mut self, key: &str, value: &str) {
        if !self.variables.contains_key(key) {
            self.variables.insert(key.to_owned(), value.to_owned());
        }
    }

    /// Removes the value of the specified variable.
    pub fn remove_value(&mut self, key: &str) {
        self.variables.remove(key);
    }

    /// Clears all variables.
    pub fn clear_variables(&mut self) {
        self.variables.clear();
    }
}

/// Information about the source of the Shell error.
#[derive(Debug, PartialEq)]
pub struct SourceInfo {
    /// The source name.
    pub src: String,
    /// The line associated with the error.
    line: usize,
    /// The column associated with the error.
    col: usize
}

impl Display for SourceInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.src, self.line, self.col)
    }
}

/// The I/O context contains the command source (i.e., file, string, telnet, etc.) and output
/// medium.
pub struct IoContext<'a> {
    /// The command source name.
    pub src: &'a str,
    /// The current line of the command source.
    pub line: usize,
    /// The current column of the command source.
    pub col: usize,
    /// The command source.
    pub input: &'a mut dyn Read,
    /// The command output.
    pub output: &'a mut dyn Write,
    buffer: [u8; 1],
}

// TODO: we need to support writing multiple formats including text and JSON
impl<'a> IoContext<'a> {
    /// Creates a new I/O context with the specified input and output.
    pub fn new(source: &'a str, input: &'a mut dyn Read, output: &'a mut dyn Write) -> Self {
        IoContext {
            src: source,
            line: 0,
            col: 0,
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

    /// Writes the specified string to the output.
    pub fn write_str(&mut self, string: &str) -> Result<(), io::Error> {
        self.output.write_all(string.as_bytes())
    }

    /// Writes the specified string to the output.
    pub fn write_string(&mut self, string: String) -> Result<(), io::Error> {
        self.output.write_all(string.as_bytes())
    }

    pub(crate) fn to_source_info(&self) -> SourceInfo {
        SourceInfo {
            src: self.src.to_string(),
            line: self.line,
            col: self.col,
        }
    }
}

/// The command context is used by the [Shell] to validate and execute commands.
///
/// The default implementation adds the following built-in commands:
/// - assign [AssignCommand]
/// - cd [CdCommand]
/// - create [CreateCommand]
/// - := [DefaultAssignCommand]
/// - echo [EchoCommand]
/// - ls [LsCommand]
/// - help [HelpCommand]
/// - mkdir [MkDirCommand]
/// - pwd [PwdCommand]
/// - source [SourceCommand]
/// - unset [UnsetCommand]
///
/// The default implementation can also invoke methods and retrieve attributes using the
/// [ExecuteCommand]
pub struct CommandContext {
    pub(crate) builtin_commands: Vec<Box<dyn Command>>,
    pub(crate) execute_command: Box<dyn Command>
}

impl Default for CommandContext {
    fn default() -> Self {
        CommandContext {
            builtin_commands: vec![Box::new(AssignCommand {}),
                                   Box::new(CdCommand {}),
                                   Box::new(CreateCommand {}),
                                   Box::new(DefaultAssignCommand {}),
                                   Box::new(EchoCommand {}),
                                   Box::new(HelpCommand {}),
                                   Box::new(LsCommand {}),
                                   Box::new(MkDirCommand {}),
                                   Box::new(PwdCommand {}),
                                   Box::new(SourceCommand {}),
                                   Box::new(UnsetCommand {})],
            execute_command: Box::new(ExecuteCommand {}),
        }
    }
}

impl CommandContext {
    /// Adds the specified to the set of specified commands that can be executed by the [Shell] with
    /// this context.
    pub fn add_command(&mut self, command: Box<dyn Command>) {
        self.builtin_commands.push(command);
    }
}