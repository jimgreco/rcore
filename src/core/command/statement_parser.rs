use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::iter::Map;
use std::str::Chars;

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) enum StatementParserErrorType {
    UnterminatedQuote,
    InvalidEscapedCharacter,
    InvalidVariableName
}

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct StatementParserError {
    pub(crate) line_num: i32,
    pub(crate) statement_num: i32,
    pub(crate) error: StatementParserErrorType
}

impl StatementParserError {
    pub(crate) fn new(
            line_num: i32,
            statement_num: i32,
            error: StatementParserErrorType) -> StatementParserError {
        StatementParserError {
            line_num,
            statement_num,
            error
        }
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct Statement {
    pub(crate) line_num: i32,
    pub(crate) statement_num: i32,
    pub(crate) words: Vec<String>,
    pub(crate) statement: String
}

impl Statement {
    pub(crate) fn new(
            line_num: i32,
            statement_num: i32,
            words: Vec<String>,
            statement: String) -> Statement {
        Statement {
            line_num,
            statement_num,
            words,
            statement
        }
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]:", self.statement_num, self.line_num)?;
        for word in self.words.iter() {
            write!(f, " \"{}\"", word)?;
        }
        Ok(())
    }
}

pub(crate) struct StatementParser<'a> {
    commands: Chars<'a>,
    context: &'a dyn CommandContext,
    line_num: i32,
    statement_num: i32
}

impl<'a> StatementParser<'a> {
    pub(crate) fn new(commands: &'a str, context: &'a dyn CommandContext) -> StatementParser<'a> {
        StatementParser {
            commands: commands.chars(),
            context,
            line_num: 1,
            statement_num: 0
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
        let mut lines = 0;
        let mut statement = String::new();

        self.statement_num += 1;

        loop {
            let opt = self.commands.next();
            if opt == None {
                // end of file
                if in_quotes {
                    return Some(Err(StatementParserError::new(
                        self.line_num, self.statement_num, StatementParserErrorType::UnterminatedQuote)));
                }

                // add the last word
                if word.len() > 0 {
                    let mut in_replace = false;
                    let mut in_replace_first = false;
                    let mut argument: usize = 0;
                    let mut variable = String::new();
                    let mut expanded = String::new();

                    for wc in word.chars() {
                        if in_replace {
                            if in_replace_first {
                                // decides whether we are in an argument or variable
                                if wc == '$' {
                                    // double dollar sign is just a dollar sign character
                                    expanded.push(wc);
                                } else if wc.is_numeric() {
                                    // arguments are all numeric
                                    argument += wc.to_digit(10).unwrap();
                                } else if wc.is_alphabetic() {
                                    // variables start with an alphabetic character and then are alphanumeric
                                    variable.push(wc);
                                } else {
                                    return Some(Err(StatementParserError::new(
                                        self.line_num, self.statement_num,
                                        StatementParserErrorType::InvalidVariableName)));
                                }
                                in_replace_first = false;
                            } else {
                                if variable.len() == 0 {
                                    // in argument
                                    if wc.is_numeric() {
                                        argument *= 10;
                                        argument += wc.to_digit(10).unwrap();
                                    } else {
                                        match self.context.get_argument(argument) {
                                            None => {
                                                return Some(Err(StatementParserError::new(
                                                    self.line_num, self.statement_num,
                                                    StatementParserErrorType::InvalidVariableName)));
                                            }
                                            Some(arg) => {
                                                expanded.push_str(arg);
                                            }
                                        }

                                        in_replace = false;
                                        argument = 0;
                                    }
                                } else {
                                    // in variable
                                    if wc.is_alphanumeric() {
                                        variable.push(wc);
                                    } else {
                                        match self.context.get_value(&variable) {
                                            None => {
                                                return Some(Err(StatementParserError::new(
                                                    self.line_num, self.statement_num,
                                                    StatementParserErrorType::InvalidVariableName)));
                                            }
                                            Some(arg) => {
                                                expanded.push_str(arg);
                                            }
                                        }

                                        in_replace = false;
                                        variable.clear();
                                    }
                                }

                                if variable.len() != 0 && wc.is_numeric()
                                        || !is_argument && wc.is_alphanumeric() {
                                    variable.push(wc);
                                } else {
                                    if is_argument {

                                    }

                                    variable.clear();
                                    is_argument = false;
                                    in_replace = false;
                                }
                            }


                            if wc.is_alphanumeric() {
                                variable.push(wc);
                            } else {
                                // done with variable
                                if variable.len() == 0 {

                                } else {
                                    let first_char = variable.chars().next().unwrap();
                                    if first_char.is_numeric() {

                                    }
                                }
                                in_replace = false;
                            }
                        } else if wc == '$' {
                            in_replace = true;
                            in_replace_first = true;
                        } else {
                            variable.push(wc);
                        }
                    }

                    words.push(word.clone());
                    word.clear();
                }
                break;
            } else {
                let c = opt.unwrap();
                statement.push(c);

                if c == '\n' {
                    // end of line
                    if in_quotes {
                        return Some(Err(StatementParserError::new(
                            self.line_num, self.statement_num, StatementParserErrorType::UnterminatedQuote)));
                    }

                    // add the last word
                    if word.len() > 0 {
                        words.push(word.clone());
                        word.clear();
                    }

                    lines += 1;

                    // continue to the next line if the last character is a backslash
                    if !in_backslash && words.len() > 0 {
                        break;
                    }

                    in_quotes = false;
                    in_comment = false;
                    in_backslash = false;
                } else if !in_comment {
                    if in_backslash {
                        if !in_quotes {
                            return Some(Err(StatementParserError::new(
                                self.line_num, self.statement_num, StatementParserErrorType::InvalidEscapedCharacter)));
                        }

                        // special characters that are escaped
                        if c == 'n' {
                            word.push('\n');
                        } else if c == 't' {
                            word.push('\t');
                        } else {
                            return Some(Err(StatementParserError::new(
                                self.line_num, self.statement_num, StatementParserErrorType::InvalidEscapedCharacter)));
                        }

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
            let statement = Statement::new(
                self.line_num, self.statement_num, words, statement);
            self.line_num += lines;
            return Some(Ok(statement));
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
    fn print_debug() {
        let statement = "create foo/bar me
        five = 123
        a \"multiple word\" statement
        a multiple line \\
        statement
        another";

        let context = SimpleContext::new();
        let result = StatementParser::new(statement, &context);
        for x in result {
            println!("{}", x.unwrap());
        }
    }

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

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::UnterminatedQuote)));
    }

    #[test]
    fn quotes_not_terminated_at_end_of_line_throws_error() {
        let statement = "foo \"bar me
        hey there";

        let result = StatementParser::new(statement, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::UnterminatedQuote)));
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

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidEscapedCharacter)));
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