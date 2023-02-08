use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::str::Chars;

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) enum StatementParserErrorType {
    UnterminatedQuote,
    InvalidEscapedCharacter,
    UnknownVariable,
    InvalidVariableFormat
}

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct StatementParserError {
    pub(crate) line_num: i32,
    pub(crate) statement_num: i32,
    pub(crate) error: StatementParserErrorType
}

impl StatementParserError {
    pub(crate) fn new(line_num: i32, statement_num: i32, error: StatementParserErrorType)
            -> StatementParserError {
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
    pub(crate) fn new(line_num: i32, statement_num: i32, words: Vec<String>, statement: String)
            -> Statement {
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
        write!(f, "{}: ", self.line_num)?;
        for word in self.words.iter() {
            write!(f, " \"{}\"", word)?;
        }
        write!(f, " [{}]", self.statement_num)
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

    fn expand(&mut self, word: &String, in_quotes: bool)
            -> Result<String, StatementParserErrorType> {
        let mut first_char = true;
        let mut in_replace = false;
        let mut in_replace_first_char = false;
        let mut argument: usize = 0;
        let mut variable = String::new();
        let mut expanded = String::new();
        let mut iterator = word.chars();
        let mut has_curly_brackets = false;

        loop {
            match iterator.next() {
                None => {
                    // end of word
                    if in_replace {
                        if in_replace_first_char || has_curly_brackets {
                            // $ at the end of a line or missing the closing curly bracket ${foo}
                            return Err(StatementParserErrorType::InvalidVariableFormat);
                        } else if variable.is_empty() {
                            // end of argument
                            match self.context.get_argument(argument) {
                                None => return Err(StatementParserErrorType::UnknownVariable),
                                Some(arg) => expanded.push_str(arg)
                            }
                        } else {
                            // end of variable
                            match self.context.get_value(&variable) {
                                None => return Err(StatementParserErrorType::UnknownVariable),
                                Some(arg) => expanded.push_str(arg)
                            }
                        }
                    }

                    // done
                    return Ok(expanded);
                }
                Some(c) => {
                    if in_replace {
                        if in_replace_first_char {
                            // first char decides whether we are in an argument or variable
                            if c == '$' {
                                if has_curly_brackets {
                                    return Err(StatementParserErrorType::InvalidVariableFormat);
                                }

                                // double dollar sign is just a dollar sign character
                                expanded.push(c);
                                in_replace = false;
                                in_replace_first_char = false;
                            } else if c.is_numeric() {
                                // arguments are all numeric
                                argument = usize::try_from(c.to_digit(10).unwrap()).unwrap();
                                in_replace_first_char = false;
                            } else if c.is_alphabetic() {
                                // variables start with an alphabetic character and then are alphanumeric
                                variable.push(c);
                                in_replace_first_char = false;
                            } else if c == '{' {
                                // skip over curly bracket, ${foo}
                                has_curly_brackets = true;
                            } else {
                                return Err(StatementParserErrorType::InvalidVariableFormat);
                            }
                        } else {
                            // second+ char
                            let mut did_expansion = false;

                            if variable.is_empty() {
                                // in argument
                                if c.is_numeric() {
                                    // shift and add numbers
                                    argument *= 10;
                                    argument += usize::try_from(c.to_digit(10).unwrap()).unwrap();
                                } else if !in_quotes && (c.is_alphabetic() || c == '$') {
                                    return Err(StatementParserErrorType::InvalidVariableFormat);
                                } else {
                                    // finished reading the argument, get the value
                                    match self.context.get_argument(argument) {
                                        None => return Err(StatementParserErrorType::UnknownVariable),
                                        Some(arg) => expanded.push_str(arg)
                                    }
                                    argument = 0;
                                    did_expansion = true;
                                }
                            } else {
                                // in variable
                                if c.is_alphanumeric() {
                                    // add to variable name
                                    variable.push(c);
                                } else if !in_quotes && c == '$' {
                                    return Err(StatementParserErrorType::InvalidVariableFormat);
                                } else {
                                    // finished reading the variable, get the value out
                                    match self.context.get_value(&variable) {
                                        None => return Err(StatementParserErrorType::UnknownVariable),
                                        Some(arg) => expanded.push_str(arg)
                                    }
                                    variable.clear();
                                    did_expansion = true;
                                }
                            }

                            if did_expansion {
                                if has_curly_brackets {
                                    if c != '}' {
                                        return Err(StatementParserErrorType::InvalidVariableFormat);
                                    }
                                    in_replace = false;
                                    has_curly_brackets = false;
                                } else if c == '$' {
                                    in_replace_first_char = true;
                                } else {
                                    in_replace = false;
                                    expanded.push(c);
                                }
                            }
                        }
                    } else if c == '$' {
                        if in_quotes || first_char {
                            in_replace = true;
                            in_replace_first_char = true;
                        } else {
                            return Err(StatementParserErrorType::InvalidVariableFormat);
                        }
                    } else {
                        // regular character
                        expanded.push(c);
                    }

                    first_char = false;
                }
            }
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
            match self.commands.next() {
                None => {
                    // end of file
                    if in_quotes {
                        return Some(Err(StatementParserError::new(
                            self.line_num, self.statement_num,
                            StatementParserErrorType::UnterminatedQuote)));
                    }

                    // add the last word
                    if !word.is_empty() {
                        match self.expand(&mut word, in_quotes) {
                            Ok(expanded) => words.push(expanded),
                            Err(e) => return Some(Err(
                                StatementParserError::new(self.line_num, self.statement_num, e)))
                        }
                        word.clear();
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
                Some(c) => {
                    statement.push(c);

                    if c == '\n' {
                        // end of line
                        if in_quotes {
                            return Some(Err(StatementParserError::new(
                                self.line_num, self.statement_num,
                                StatementParserErrorType::UnterminatedQuote)));
                        }

                        // add the last word
                        if !word.is_empty() {
                            match self.expand(&word, in_quotes) {
                                Ok(expanded) => words.push(expanded),
                                Err(e) => return Some(Err(
                                    StatementParserError::new(self.line_num, self.statement_num, e)))
                            }
                            word.clear();
                        }
                        lines += 1;

                        // continue to the next line if the last character is a backslash
                        if !in_backslash && words.len() > 0 {
                            let statement = Statement::new(
                                self.line_num, self.statement_num, words, statement);
                            self.line_num += lines;
                            return Some(Ok(statement));
                        }

                        in_quotes = false;
                        in_comment = false;
                        in_backslash = false;
                    } else if !in_comment {
                        if in_backslash {
                            if !in_quotes {
                                return Some(Err(StatementParserError::new(
                                    self.line_num, self.statement_num,
                                    StatementParserErrorType::InvalidEscapedCharacter)));
                            }

                            // special characters that are escaped
                            if c == 'n' {
                                word.push('\n');
                            } else if c == '\\' {
                                word.push('\\');
                            } else if c == '"' {
                                word.push('"');
                            } else {
                                return Some(Err(StatementParserError::new(
                                    self.line_num, self.statement_num,
                                    StatementParserErrorType::InvalidEscapedCharacter)));
                            }

                            in_backslash = false;
                        } else if c == '\\' {
                            // escape the next character or continue to the next line
                            in_backslash = true;
                        } else if c == '"' {
                            if in_quotes {
                                // end quotes
                                // include zero length words
                                match self.expand(&word, in_quotes) {
                                    Ok(expanded) => words.push(expanded),
                                    Err(e) => return Some(Err(
                                        StatementParserError::new(self.line_num, self.statement_num, e)))
                                }
                                word.clear();
                            }
                            in_quotes = !in_quotes
                        } else if c == '#' && !in_quotes {
                            // start of comment
                            // add the last word
                            if !word.is_empty() {
                                match self.expand(&word, in_quotes) {
                                    Ok(expanded) => words.push(expanded),
                                    Err(e) => return Some(Err(
                                        StatementParserError::new(self.line_num, self.statement_num, e)))
                                }
                                word.clear();
                            }

                            in_comment = true;
                        } else if c.is_whitespace() {
                            if in_quotes {
                                // include all whitespace in quotes
                                word.push(c);
                            } else if !word.is_empty() {
                                // end the current word
                                match self.expand(&word, in_quotes) {
                                    Ok(expanded) => words.push(expanded),
                                    Err(e) => return Some(Err(
                                        StatementParserError::new(self.line_num, self.statement_num, e)))
                                }
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
        }
    }
}

pub trait CommandContext {
    fn get_argument(&self, position: usize) -> Option<&String>;
    fn get_value(&self, key: &str) -> Option<&String>;
    fn add_argument(&mut self, value: &str);
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

    fn add_argument(&mut self, value: &str) {
        self.arguments.push(value.to_string());
    }

    fn set_value(&mut self, key: &str, value: &str, force: bool) {
        if force || !self.variables.contains_key(key) {
            self.variables.insert(key.to_string(), value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::command::CommandContext;
    use crate::core::command::statement_parser::{StatementParser, StatementParserError, StatementParserErrorType, SimpleContext};

    #[test]
    fn print_debug_statements() {
        let statement = "create $wtf/bar me
        five = 123
        a \"multiple word\" statement
        a $1 multiple line \\
        statement
        another";
        let mut context = SimpleContext::new();
        context.add_argument("testing123");
        context.add_argument("testing456");
        context.set_value("wtf", "foo", true);

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
    fn invalid_escaped_character_throws_error() {
        let statement = "foo \"b\\^br\" me";

        let result = StatementParser::new(statement, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidEscapedCharacter)));
    }

    #[test]
    fn escaped_characters() {
        let statement = "foo \"bar \\n me \\\" now \\\\ abc \"";

        let statements = StatementParser::new(statement, &SimpleContext::new()).next().unwrap();

        assert_eq!(statements.unwrap().words[1], "bar \n me \" now \\ abc ");
    }

    #[test]
    fn single_statement_is_processed() {
        let statement = "foo bar";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "foo_/_bar");
    }

    #[test]
    fn multiple_statements_are_processed() {
        let statement = "Jojo was a man who thought he was a loner
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner");
        assert_eq!(statements[1], "But_/_he_/_knew it couldn't_/_last");
        assert_eq!(statements[2], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn empty_statements_are_ignored() {
        let statement = "Jojo was a man who thought he was a loner

        But he \"knew it couldn't\" last

        \"Jojo left his home\" in \"Tuscon, Arizona\"

        ";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner");
        assert_eq!(statements[1], "But_/_he_/_knew it couldn't_/_last");
        assert_eq!(statements[2], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn whitespace_is_ignored() {
        let statement = "   foo     bar  ";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0], "foo_/_bar");
    }

    #[test]
    fn backslash_will_continue_statement_on_the_next_line() {
        let statement = "Jojo was a man who thought he was a loner \
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner_/_But_/_he_/_knew it couldn't_/_last");
        assert_eq!(statements[1], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn empty_quotes_can_be_a_word() {
        let statement = "foo \"\" bar";
        let context = SimpleContext::new();
        let parser = StatementParser::new(statement, &context);

        let statements: Vec<Vec<String>> = parser.map(|r| r.unwrap().words).collect();

        let statement = &statements[0];
        assert_eq!(statement.len(), 3);
        assert_eq!(statement[0], "foo");
        assert_eq!(statement[1], "");
        assert_eq!(statement[2], "bar");
    }

    #[test]
    fn backslash_not_in_quotes_is_error() {
        let statement = "back\\\"slash";

        let result = StatementParser::new(statement, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidEscapedCharacter)));
    }

    #[test]
    fn comment_at_start_of_line_removes_statement() {
        let statement = "Jojo was a man who thought he was a loner
        #But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 2);
        assert_eq!(statements[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner");
        assert_eq!(statements[1], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn comment_at_end_of_line_removes_remaining_content() {
        let statement = "Jojo was a man # who thought he was a loner
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let statements: Vec<String> = StatementParser::new(statement, &SimpleContext::new()).map(
            |r| r.unwrap().words.join("_/_")).collect();

        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0], "Jojo_/_was_/_a_/_man");
        assert_eq!(statements[1], "But_/_he_/_knew it couldn't_/_last");
        assert_eq!(statements[2], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn one_position_argument() {
        let statement = "$0";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo left his home");

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left his home")
    }

    #[test]
    fn position_argument_outside_quotes_is_error() {
        let statement = "Jojo$0";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let result = StatementParser::new(statement, &context).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn multiple_position_arguments_not_in_quotes_is_an_error() {
        let statement = "$0$1";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo");
        context.add_argument(" left his home");

        let result = StatementParser::new(statement, &context).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn multiple_position_arguments_inside_quotes() {
        let statement = "\"$0$1\"";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo");
        context.add_argument(" left his home");

        let result = StatementParser::new(statement, &context).next().unwrap();

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left his home");
    }

    #[test]
    fn position_argument_can_be_anywhere_in_string_when_inside_quotes() {
        let statement = "\"Jojo$0\"";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left his home")
    }

    #[test]
    fn unknown_position_argument_is_error() {
        let statement = "$1";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let result = StatementParser::new(statement, &context).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::UnknownVariable)));
    }

    #[test]
    fn multiple_position_arguments() {
        let statement = "\"$0 $1\" $2";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo");
        context.add_argument("left");
        context.add_argument("his home");

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left");
        assert_eq!(statement[0][1], "his home");
    }

    #[test]
    fn curly_brackets_can_be_used_to_separate_position_arguments_from_numbers() {
        let statement = "\"${0}345\"";
        let mut context = SimpleContext::new();
        context.add_argument("12");

        let result = StatementParser::new(statement, &context).next().unwrap();

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "12345");
    }

    #[test]
    fn one_variable() {
        let statement = "$foo";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left his home", true);

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left his home")
    }

    #[test]
    fn variable_outside_quotes_is_error() {
        let statement = "Jojo$foo";
        let mut context = SimpleContext::new();
        context.set_value("foo", " left his home", true);

        let result = StatementParser::new(statement, &context).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn variable_can_be_anywhere_in_string_when_inside_quotes() {
        let statement = "\"Jojo$foo\"";
        let mut context = SimpleContext::new();
        context.set_value("foo", " left his home", true);

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left his home")
    }

    #[test]
    fn unknown_variable_is_error() {
        let statement = "$1";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let result = StatementParser::new(statement, &context).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::UnknownVariable)));
    }

    #[test]
    fn multiple_variables() {
        let statement = "\"$foo $bar\" $me";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo", true);
        context.set_value("bar", "left", true);
        context.set_value("me", "his home", true);

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left");
        assert_eq!(statement[0][1], "his home");
    }

    #[test]
    fn multiple_variables_not_in_quotes_is_an_error() {
        let statement = "$foo$bar";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left", true);
        context.set_value("bar", " his home", true);

        let result = StatementParser::new(statement, &context).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn multiple_variables_inside_quotes() {
        let statement = "\"$foo$bar\"";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left", true);
        context.set_value("bar", " his home", true);

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left his home");
    }

    #[test]
    fn curly_brackets_can_be_used_to_separate_variables_from_text() {
        let statement = "\"${foo}his home\"";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left ", true);

        let statement: Vec<Vec<String>> = StatementParser::new(statement, &context).map(
            |r| r.unwrap().words).collect();

        assert_eq!(statement[0][0], "Jojo left his home");
    }

    #[test]
    fn unterminated_curly_bracket_is_error() {
        let statement = "${foo";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left ", true);

        let result = StatementParser::new(statement, &context).next().unwrap();

        assert_eq!(result, Err(StatementParserError::new(1, 1, StatementParserErrorType::InvalidVariableFormat)));
    }
}