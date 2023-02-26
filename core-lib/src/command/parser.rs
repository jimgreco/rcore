use crate::command::lexer::{Lexer, LexerContext, LexerErrorType, SimpleContext};

pub trait ExecutableCommand {
    fn execute(&self, context: &mut dyn LexerContext);
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

impl ExecutableCommand for AssignmentCommand {
    fn execute(&self, context: &mut dyn LexerContext) {
        context.set_value(&self.words[0], &self.words[2], self.words[1] == "=");
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub enum ParserErrorType {
    LexerUnterminatedQuote,
    LexerInvalidEscapedCharacter,
    LexerUnknownVariable,
    LexerInvalidVariableFormat
}

pub struct ParserError<'a> {
    error_type: ParserErrorType,
    line_number: i32,
    statement_number: i32,
    line: String,
    file: &'a str
}

impl<'a> ParserError<'a> {
    pub fn new(error_type: ParserErrorType,
               line_number: i32,
               statement_number: i32,
               file: &str) -> ParserError {
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
        ParserError { error_type, line_number, statement_number, line, file }
    }
}

pub struct Parser<'a> {
    file: &'a str
}

impl<'a> Parser<'a> {
    pub fn new(file: &'a mut str) -> Parser {
        Parser {
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
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Box<dyn ExecutableCommand>, ParserError<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let context = SimpleContext::new();
        let statement_parser = Lexer::new(self.file, &context);
        let mut statement_number = 1;

        for statement_result in statement_parser {
            match statement_result {
                Ok(statement) => {
                    let words = statement.tokens;

                    if AssignmentCommand::is_command(&words) {
                        if !self.validate_variable(&words[0]) {
                            return Some(Err(ParserError::new(
                                ParserErrorType::LexerUnknownVariable,
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
                        LexerErrorType::UnterminatedQuote =>
                            ParserErrorType::LexerUnterminatedQuote,
                        LexerErrorType::InvalidEscapedCharacter =>
                            ParserErrorType::LexerInvalidEscapedCharacter,
                        LexerErrorType::UnknownVariable =>
                            ParserErrorType::LexerUnknownVariable,
                        LexerErrorType::InvalidVariableFormat =>
                            ParserErrorType::LexerInvalidVariableFormat
                    };

                    return Some(Err(ParserError::new(
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
