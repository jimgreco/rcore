use crate::core::command::CommandContext;
use crate::core::command::statement_parser::{SimpleContext, StatementParser, StatementParserErrorType};

pub trait Command {
    fn execute(&self, context: &mut dyn CommandContext);
}

pub struct AssignmentCommand {
    words: Vec<String>
}

impl AssignmentCommand {
    fn is_command(words: &Vec<String>) -> bool {
        return words.len() == 3 && (words[1] == "=" || words[1] == ":=");
    }

    pub fn new(words: Vec<String>) -> AssignmentCommand {
        AssignmentCommand {
            words
        }
    }
}

impl Command for AssignmentCommand {
    fn execute(&self, context: &mut dyn CommandContext) {
        context.set_value(&self.words[0], &self.words[2], self.words[1] == "=");
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub enum CommandParserErrorType {
    UnterminatedQuote,
    InvalidEscapedCharacter,
    InvalidVariableName,
    UnknownPositionVariable,
    UnknownVariableName
}

pub struct CommandParserError<'a> {
    error_type: CommandParserErrorType,
    line_number: i32,
    statement_number: i32,
    line: String,
    file: &'a str
}

impl<'a> CommandParserError<'a> {
    pub fn new(error_type: CommandParserErrorType,
               line_number: i32,
               statement_number: i32,
               file: &str) -> CommandParserError {
        let mut current_line = 1;
        let mut start = 0;
        let mut end = 0;

        for (i, c) in file.chars().enumerate() {
            if c == '\n' {
                if current_line == line_number {
                    end = i + 1;
                    break;
                }
                current_line += 1;
                if current_line == line_number {
                    start = i + 1;
                }
            }
        }

        let line: String = file.chars().skip(start).take(end - start).collect();
        CommandParserError { error_type, line_number, statement_number, line, file }
    }
}

pub struct CommandParser<'a> {
    file: &'a str
}

impl<'a> CommandParser<'a> {
    pub fn new(file: &'a mut str) -> CommandParser {
        CommandParser {
            file
        }
    }

    fn validate_variable(&mut self, variable: &str) -> bool {
        let mut first = true;

        for c in variable.chars() {
            if first && !c.is_alphabetic() {
                return false;
            } else if !c.is_alphanumeric() {
                return false;
            }
            first = false;
        }

        return true;
    }

    fn expand_value(&mut self, value: &str) -> Result<String, CommandParserErrorType> {
        let mut expanded_variable = String::new();
        let mut variable = String::new();
        let mut in_variable = true;
        let mut in_backslash = true;

        for c in value.chars() {
            if in_variable {
                if c.is_alphanumeric() {
                    variable.push(c);
                }
            } else if in_backslash {
                if ['\\', 'n', '$', '@'].contains(&c) {
                    expanded_variable.push(c);
                    in_backslash = false;
                } else {
                    return Err(CommandParserErrorType::InvalidEscapedCharacter);
                }
            } else if c == '$' {
                in_variable = true;
            }
        }

        Ok(expanded_variable)
    }
}

impl<'a> Iterator for CommandParser<'a> {
    type Item = Result<Box<dyn Command>, CommandParserError<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let context = SimpleContext::new();
        let statement_parser = StatementParser::new(self.file, &context);
        let mut statement_number = 1;

        for statement_result in statement_parser {
            match statement_result {
                Ok(statement) => {
                    let words = statement.words;

                    if AssignmentCommand::is_command(&words) {
                        if !self.validate_variable(&words[0]) {
                            return Some(Err(CommandParserError::new(
                                CommandParserErrorType::InvalidVariableName,
                                statement.line_num,
                                statement_number,
                                self.file
                            )));
                        }

                        return Some(Ok(Box::new(AssignmentCommand::new(words))));
                    }

                    statement_number += 1;
                }
                Err(statement_error) => {
                    let command_error_type = match statement_error.error {
                        StatementParserErrorType::UnterminatedQuote =>
                            CommandParserErrorType::UnterminatedQuote,
                        StatementParserErrorType::InvalidEscapedCharacter =>
                            CommandParserErrorType::InvalidEscapedCharacter,
                        StatementParserErrorType::InvalidVariableName =>
                            CommandParserErrorType::InvalidVariableName
                    };

                    return Some(Err(CommandParserError::new(
                        command_error_type,
                        statement_error.line_num,
                        statement_number,
                        self.file
                    )));
                }
            }
        }

        None
    }
}
