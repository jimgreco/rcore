use std::collections::HashMap;
use std::iter::Map;
use std::str::Chars;

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) enum StatementParserErrorType {
    UnterminatedQuote,
    InvalidEscapedCharacter,
}

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct StatementParserError {
    pub(crate) line_number: i32,
    pub(crate) error_type: StatementParserErrorType
}

impl StatementParserError {
    pub(crate) fn new(line_number: i32, error_type: StatementParserErrorType)
            -> StatementParserError {
        StatementParserError {
            line_number,
            error_type
        }
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct Statement {
    pub(crate) line_number: i32,
    pub(crate) words: Vec<String>
}

impl Statement {
    pub(crate) fn new(line_number: i32, words: Vec<String>) -> Statement {
        Statement {
            line_number,
            words
        }
    }
}

pub(crate) struct StatementParser<'a> {
    commands: &'a mut Chars<'a>,
    context: &'a dyn CommandContext,
    line_number: i32
}

impl<'a> StatementParser<'a> {
    pub(crate) fn new(commands: &'a str, context: &'a dyn CommandContext) -> StatementParser<'a> {
        StatementParser {
            commands: &mut commands.chars(),
            context,
            line_number: 1
        }
    }
}

impl<'a> Iterator for StatementParser<'a> {
    type Item = Result<Statement, StatementParserError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut in_quotes = false;
        let mut in_comment = false;
        let mut in_backslash = false;
        let mut word = String::new();
        let mut words: Vec<String> = Vec::new();

        loop {
            let opt = self.commands.next();
            if opt == None {
                // end of file
                if in_quotes {
                    return Some(Err(StatementParserError::new(
                        self.line_number, StatementParserErrorType::UnterminatedQuote)));
                }

                // add the last word
                if word.len() > 0 {
                    words.push(word.clone());
                    word.clear();
                }
                break;
            } else {
                let c = opt.unwrap();

                if c == '\n' {
                    // end of line
                    if in_quotes {
                        return Some(Err(StatementParserError::new(
                            self.line_number, StatementParserErrorType::UnterminatedQuote)));
                    }

                    // add the last word
                    if word.len() > 0 {
                        words.push(word.clone());
                        word.clear();
                    }

                    // continue to the next line if the last character is a backslash
                    if !in_backslash && words.len() > 0 {
                        break;
                    }

                    self.line_number += 1;
                    in_quotes = false;
                    in_comment = false;
                    in_backslash = false;
                } else if !in_comment {
                    if in_backslash {
                        if !in_quotes {
                            return Some(Err(StatementParserError::new(
                                self.line_number, StatementParserErrorType::InvalidEscapedCharacter)));
                        }

                        // special characters that are escaped
                        word.push('\\');
                        word.push(c);

                        in_backslash = false;
                    } else if c == '\\' {
                        // escape the next character or continue to the next line
                        in_backslash = true;
                    } else if c == '"' {
                        if in_quotes {
                            // end quotes
                            // include zero length words
                            words.push(word.clone());
                            word.clear();

                            in_quotes = false;
                        } else {
                            // start quotes
                            in_quotes = true;
                        }
                    } else if c == '#' {
                        // start of comment
                        if word.len() > 0 {
                            words.push(word.clone());
                            word.clear();
                        }

                        in_comment = true;
                    } else if c.is_whitespace() {
                        if in_quotes {
                            // include all whitespace in quotes
                            word.push(c);
                        } else if word.len() > 0 {
                            // end the current word
                            words.push(word.clone());
                            word.clear();
                        }
                        // otherwise, ignore whitespace
                    } else {
                        // add to the current word
                        word.push(c);
                    }
                }
            }
        }

        if words.len() > 0 {
            return Some(Ok(Statement::new(self.line_number, words)));
        } else {
            return None;
        }
    }
}

pub trait CommandContext {
    fn get_argument(&self, position: usize) -> Option<&String>;
    fn get_value(&self, key: &str) -> Option<&String>;
    fn set_value(&mut self, key: &str, value: &str, force: bool);
}

pub(crate) struct SimpleContext {
    variables: HashMap<String, String>,
    arguments: Vec<String>,
}

impl SimpleContext {
    pub(crate) fn new() -> SimpleContext {
        SimpleContext {
            variables: HashMap::new(),
            arguments: Vec::new()
        }
    }
}

impl CommandContext for SimpleContext {
    fn get_argument(&self, position: usize) -> Option<&String> {
        self.arguments.get(position)
    }

    fn get_value(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    fn set_value(&mut self, key: &str, value: &str, force: bool) {
        if force || !self.variables.contains_key(key) {
            self.variables.insert(key.to_string(), value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::command::statement_parser::{StatementParser, StatementParserError, StatementParserErrorType, CommandContext, SimpleContext};

    #[test]
    fn no_statements_returns_none() {
        let statement = "


        ";

        let result = StatementParser::new(statement, &SimpleContext::new()).next();

        assert_eq!(result, None);
    }

    #[test]
    fn quotes_not_terminated_at_end_of_file_throws_error() {
        let statement = "foo \"bar me";

        let result = StatementParser::new(statement, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, StatementParserErrorType::UnterminatedQuote)));
    }

    #[test]
    fn quotes_not_terminated_at_end_of_line_throws_error() {
        let statement = "foo \"bar me
        hey there";

        let result = StatementParser::new(statement, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, StatementParserErrorType::UnterminatedQuote)));
    }

    #[test]
    fn single_statement_is_processed() {
        let statement = "context $1";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "context_/_$1");
    }

    #[test]
    fn multiple_statements_are_processed() {
        let statement = "context $1
        create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
        create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0], "context_/_$1");
        assert_eq!(statements[1], "create_/_$1_/_com.core.platform.applications.sequencer.Sequencer_/_@$3_/_SEQ01");
        assert_eq!(statements[2], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
    }

    #[test]
    fn empty_statement_is_ignored() {
        let statement = "context $1

        create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
        create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0], "context_/_$1");
        assert_eq!(statements[1], "create_/_$1_/_com.core.platform.applications.sequencer.Sequencer_/_@$3_/_SEQ01");
        assert_eq!(statements[2], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
    }

    #[test]
    fn empty_statement_at_end_is_ignored() {
        let statement = "context $1

        create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
        create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3

        ";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0], "context_/_$1");
        assert_eq!(statements[1], "create_/_$1_/_com.core.platform.applications.sequencer.Sequencer_/_@$3_/_SEQ01");
        assert_eq!(statements[2], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
    }

    #[test]
    fn whitespace_is_ignored() {
        let statement = "   context     $1  ";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "context_/_$1");
    }

    #[test]
    fn backslash_will_continue_statement_on_the_next_line() {
        let statement = "   context     $1  \
        create $1 com.core.platform.applications.sequencer.Sequencer     @$3     SEQ01
        create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3
        ";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "context_/_$1_/_create_/_$1_/_com.core.platform.applications.sequencer.Sequencer_/_@$3_/_SEQ01");
        assert_eq!(statements[1], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3")
    }

    #[test]
    fn quotes_will_mark_a_string() {
        let statement = "soo    \"foo bar me\"   do";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "soo_/_foo bar me_/_do");
    }

    #[test]
    fn empty_quotes_can_be_a_word() {
        let statement = "foo \"\" bar";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "foo_/__/_bar");
    }

    #[test]
    fn backslash_can_represent_special_characters() {
        let statement = "\"backslash\\\\\" \"newline\\n\" \"pound\\#\" \"dollarsign\\$\" \"quotes\\\"\"";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "backslash\\\\_/_newline\\n_/_pound\\#_/_dollarsign\\$_/_quotes\\\"");
    }

    #[test]
    fn backslash_not_in_quotes_is_error() {
        let statement = "back\\\"slash";

        let result = StatementParser::new(statement, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, StatementParserErrorType::InvalidEscapedCharacter)));
    }

    #[test]
    fn comment_at_start_of_line_removes_statement() {
        let statement = "context $1
        #create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
        create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "context_/_$1");
        assert_eq!(statements[1], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
    }

    #[test]
    fn command_at_end_of_line_removes_remaining_content() {
        let statement = "context # $1
        #create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
        create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "context");
        assert_eq!(statements[1], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
    }
}