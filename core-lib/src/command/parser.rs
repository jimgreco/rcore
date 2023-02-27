use crate::command::lexer::{Lexer, LexerContext, LexerError, SimpleContext};

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

#[derive(Debug, PartialEq)]
pub enum ParserError {
    LexerError(LexerError)
}

pub struct Parser<'a> {
    file: &'a str,
    context: &'a dyn LexerContext,
    lexer: Lexer<'a>
}

impl<'a> Parser<'a> {
    pub fn new(file: &'a str, context: &'a dyn LexerContext) -> Parser {
        Parser {
            file,
            context,
            lexer: Lexer::new()
        }
    }

    fn validate_variable(&self, variable: &str) -> bool {
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
    type Item = Result<Box<dyn ExecutableCommand>, ParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        let lexer = Lexer::new(self.file, self.context);
        let mut statement_number = 1;

        for statement_result in lexer {
            match statement_result {
                Ok(statement) => {
                    let words = statement.tokens;

                    if AssignmentCommand::is_command(&words) {
                        if !self.validate_variable(&words[0]) {
                            return Some(Err(ParserError::new(
                                ParserError::LexerUnknownVariable,
                                statement.line,
                                statement_number,
                                self.file
                            )));
                        }

                        return Some(Ok(Box::new(AssignmentCommand::new(words))));
                    }

                    statement_number += 1;
                }
                Err(statement_error) => {
                    let command_error_type = match statement_error.reason {
                        LexerError::UnterminatedQuote =>
                            ParserError::LexerUnterminatedQuote,
                        LexerError::InvalidEscapedCharacterFormat =>
                            ParserError::LexerInvalidEscapedCharacter,
                        LexerError::UnknownVariable =>
                            ParserError::LexerUnknownVariable,
                        LexerError::InvalidVariableFormat =>
                            ParserError::LexerInvalidVariableFormat
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
