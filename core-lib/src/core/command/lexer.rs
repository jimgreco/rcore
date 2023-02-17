use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::str::Chars;

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) enum LexerErrorType {
    UnterminatedQuote,
    InvalidEscapedCharacter,
    UnknownVariable,
    InvalidVariableFormat
}

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct LexerError {
    pub(crate) line_num: i32,
    pub(crate) command_num: i32,
    pub(crate) error: LexerErrorType
}

impl LexerError {
    pub(crate) fn new(line_num: i32, command_num: i32, error: LexerErrorType) -> LexerError {
        LexerError { line_num, command_num, error }
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub(crate) struct Command {
    pub(crate) line_num: i32,
    pub(crate) command_num: i32,
    pub(crate) tokens: Vec<String>,
    pub(crate) text: String
}

impl Command {
    pub(crate) fn new(line_num: i32, command_num: i32, tokens: Vec<String>, text: String)
            -> Command {
        Command { line_num, command_num, tokens, text }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.line_num)?;
        for token in self.tokens.iter() {
            write!(f, " \"{}\"", token)?;
        }
        write!(f, " [{}]", self.command_num)
    }
}

pub(crate) struct Lexer<'a> {
    commands: Chars<'a>,
    context: &'a dyn LexerContext,
    line_num: i32,
    command_num: i32
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(commands: &'a str, context: &'a dyn LexerContext) -> Lexer<'a> {
        Lexer {
            commands: commands.chars(),
            context,
            line_num: 1,
            command_num: 0
        }
    }

    fn expand(&mut self, token: &String, in_quotes: bool) -> Result<String, LexerErrorType> {
        let mut first_char = true;
        let mut in_replace = false;
        let mut in_replace_first_char = false;
        let mut argument: usize = 0;
        let mut variable = String::new();
        let mut expanded = String::new();
        let mut iterator = token.chars();
        let mut has_curly_brackets = false;

        loop {
            match iterator.next() {
                None => {
                    // end of token
                    if in_replace {
                        if in_replace_first_char || has_curly_brackets {
                            // $ at the end of a line or missing the closing curly bracket ${foo}
                            return Err(LexerErrorType::InvalidVariableFormat);
                        } else if variable.is_empty() {
                            // end of argument
                            match self.context.get_argument(argument) {
                                None => return Err(LexerErrorType::UnknownVariable),
                                Some(arg) => expanded.push_str(arg)
                            }
                        } else {
                            // end of variable
                            match self.context.get_value(&variable) {
                                None => return Err(LexerErrorType::UnknownVariable),
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
                                    return Err(LexerErrorType::InvalidVariableFormat);
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
                                return Err(LexerErrorType::InvalidVariableFormat);
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
                                    return Err(LexerErrorType::InvalidVariableFormat);
                                } else {
                                    // finished reading the argument, get the value
                                    match self.context.get_argument(argument) {
                                        None => return Err(LexerErrorType::UnknownVariable),
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
                                    return Err(LexerErrorType::InvalidVariableFormat);
                                } else {
                                    // finished reading the variable, get the value out
                                    match self.context.get_value(&variable) {
                                        None => return Err(LexerErrorType::UnknownVariable),
                                        Some(arg) => expanded.push_str(arg)
                                    }
                                    variable.clear();
                                    did_expansion = true;
                                }
                            }

                            if did_expansion {
                                if has_curly_brackets {
                                    if c != '}' {
                                        return Err(LexerErrorType::InvalidVariableFormat);
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
                            return Err(LexerErrorType::InvalidVariableFormat);
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

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Command, LexerError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut in_quotes = false;
        let mut in_comment = false;
        let mut in_backslash = false;
        let mut token = String::new();
        let mut tokens: Vec<String> = Vec::new();
        let mut lines = 0;
        let mut text = String::new();

        self.command_num += 1;

        loop {
            match self.commands.next() {
                None => {
                    // end of file
                    if in_quotes {
                        return Some(Err(LexerError::new(
                            self.line_num, self.command_num,
                            LexerErrorType::UnterminatedQuote)));
                    }

                    // add the last token
                    if !token.is_empty() {
                        match self.expand(&mut token, in_quotes) {
                            Ok(expanded) => tokens.push(expanded),
                            Err(e) => return Some(Err(
                                LexerError::new(self.line_num, self.command_num, e)))
                        }
                        token.clear();
                    }

                    if tokens.len() > 0 {
                        let command = Command::new(
                            self.line_num, self.command_num, tokens, text);
                        self.line_num += lines;
                        return Some(Ok(command));
                    } else {
                        return None;
                    }
                }
                Some(c) => {
                    text.push(c);

                    if c == '\n' {
                        // end of line
                        if in_quotes {
                            return Some(Err(LexerError::new(
                                self.line_num, self.command_num,
                                LexerErrorType::UnterminatedQuote)));
                        }

                        // add the last token
                        if !token.is_empty() {
                            match self.expand(&token, in_quotes) {
                                Ok(expanded) => tokens.push(expanded),
                                Err(e) => return Some(Err(
                                    LexerError::new(self.line_num, self.command_num, e)))
                            }
                            token.clear();
                        }
                        lines += 1;

                        // continue to the next line if the last character is a backslash
                        if !in_backslash && tokens.len() > 0 {
                            let command = Command::new(
                                self.line_num, self.command_num, tokens, text);
                            self.line_num += lines;
                            return Some(Ok(command));
                        }

                        in_quotes = false;
                        in_comment = false;
                        in_backslash = false;
                    } else if !in_comment {
                        if in_backslash {
                            if !in_quotes {
                                return Some(Err(LexerError::new(
                                    self.line_num, self.command_num,
                                    LexerErrorType::InvalidEscapedCharacter)));
                            }

                            // special characters that are escaped
                            if c == 'n' {
                                token.push('\n');
                            } else if c == '\\' {
                                token.push('\\');
                            } else if c == '"' {
                                token.push('"');
                            } else {
                                return Some(Err(LexerError::new(
                                    self.line_num, self.command_num,
                                    LexerErrorType::InvalidEscapedCharacter)));
                            }

                            in_backslash = false;
                        } else if c == '\\' {
                            // escape the next character or continue to the next line
                            in_backslash = true;
                        } else if c == '"' {
                            if in_quotes {
                                // end quotes
                                // include zero length tokens
                                match self.expand(&token, in_quotes) {
                                    Ok(expanded) => tokens.push(expanded),
                                    Err(e) => return Some(Err(
                                        LexerError::new(self.line_num, self.command_num, e)))
                                }
                                token.clear();
                            }
                            in_quotes = !in_quotes
                        } else if c == '#' && !in_quotes {
                            // start of comment
                            // add the last token
                            if !token.is_empty() {
                                match self.expand(&token, in_quotes) {
                                    Ok(expanded) => tokens.push(expanded),
                                    Err(e) => return Some(Err(
                                        LexerError::new(self.line_num, self.command_num, e)))
                                }
                                token.clear();
                            }

                            in_comment = true;
                        } else if c.is_whitespace() {
                            if in_quotes {
                                // include all whitespace in quotes
                                token.push(c);
                            } else if !token.is_empty() {
                                // end the current token
                                match self.expand(&token, in_quotes) {
                                    Ok(expanded) => tokens.push(expanded),
                                    Err(e) => return Some(Err(
                                        LexerError::new(self.line_num, self.command_num, e)))
                                }
                                token.clear();
                            }
                            // otherwise, ignore whitespace
                        } else {
                            // add to the current token
                            token.push(c);
                        }
                    }
                }
            }
        }
    }
}

pub trait LexerContext {
    fn get_argument(&self, position: usize) -> Option<&String>;
    fn get_value(&self, key: &str) -> Option<&String>;
    fn add_argument(&mut self, value: &str);
    fn set_value(&mut self, key: &str, value: &str, force: bool);
}

impl LexerContext for SimpleContext {
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

#[cfg(test)]
mod tests {
    use crate::core::command::lexer::{Lexer, LexerContext, LexerError, LexerErrorType, SimpleContext};

    #[test]
    fn print_debug_command() {
        let text = "create $wtf/bar me
        five = 123
        a \"multiple token\" command
        a $1 multiple line \\
        command
        another";
        let mut context = SimpleContext::new();
        context.add_argument("testing123");
        context.add_argument("testing456");
        context.set_value("wtf", "foo", true);

        let result = Lexer::new(text, &context);

        for x in result {
            println!("{}", x.unwrap());
        }
    }

    #[test]
    fn no_commands_returns_none() {
        let text = "


        ";

        let result = Lexer::new(text, &SimpleContext::new()).next();

        assert_eq!(result, None);
    }

    #[test]
    fn quotes_not_terminated_at_end_of_file_throws_error() {
        let text = "foo \"bar me";

        let result = Lexer::new(text, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::UnterminatedQuote)));
    }

    #[test]
    fn quotes_not_terminated_at_end_of_line_throws_error() {
        let text = "foo \"bar me
        hey there";

        let result = Lexer::new(text, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::UnterminatedQuote)));
    }

    #[test]
    fn invalid_escaped_character_throws_error() {
        let text = "foo \"b\\^br\" me";

        let result = Lexer::new(text, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::InvalidEscapedCharacter)));
    }

    #[test]
    fn escaped_characters() {
        let text = "foo \"bar \\n me \\\" now \\\\ abc \"";

        let commands = Lexer::new(text, &SimpleContext::new()).next().unwrap();

        assert_eq!(commands.unwrap().tokens[1], "bar \n me \" now \\ abc ");
    }

    #[test]
    fn single_command_is_processed() {
        let text = "foo bar";

        let commands: Vec<String> = Lexer::new(text, &SimpleContext::new()).map(
            |r| r.unwrap().tokens.join("_/_")).collect();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "foo_/_bar");
    }

    #[test]
    fn multiple_commands_are_processed() {
        let text = "Jojo was a man who thought he was a loner
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let commands: Vec<String> = Lexer::new(text, &SimpleContext::new()).map(
            |r| r.unwrap().tokens.join("_/_")).collect();

        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner");
        assert_eq!(commands[1], "But_/_he_/_knew it couldn't_/_last");
        assert_eq!(commands[2], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn empty_commands_are_ignored() {
        let text = "Jojo was a man who thought he was a loner

        But he \"knew it couldn't\" last

        \"Jojo left his home\" in \"Tuscon, Arizona\"

        ";

        let commands: Vec<String> = Lexer::new(text, &SimpleContext::new()).map(
            |r| r.unwrap().tokens.join("_/_")).collect();

        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner");
        assert_eq!(commands[1], "But_/_he_/_knew it couldn't_/_last");
        assert_eq!(commands[2], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn whitespace_is_ignored() {
        let text = "   foo     bar  ";

        let commands: Vec<String> = Lexer::new(text, &SimpleContext::new()).map(
            |r| r.unwrap().tokens.join("_/_")).collect();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "foo_/_bar");
    }

    #[test]
    fn backslash_will_continue_command_on_the_next_line() {
        let text = "Jojo was a man who thought he was a loner \
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let commands: Vec<String> = Lexer::new(text, &SimpleContext::new()).map(
            |r| r.unwrap().tokens.join("_/_")).collect();

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner_/_But_/_he_/_knew it couldn't_/_last");
        assert_eq!(commands[1], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn empty_quotes_can_be_a_token() {
        let text = "foo \"\" bar";
        let context = SimpleContext::new();
        let lexer = Lexer::new(text, &context);

        let commands: Vec<Vec<String>> = lexer.map(|r| r.unwrap().tokens).collect();

        let tokens = &commands[0];
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], "foo");
        assert_eq!(tokens[1], "");
        assert_eq!(tokens[2], "bar");
    }

    #[test]
    fn backslash_not_in_quotes_is_error() {
        let text = "back\\\"slash";

        let result = Lexer::new(text, &SimpleContext::new()).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::InvalidEscapedCharacter)));
    }

    #[test]
    fn comment_at_start_of_line_removes_command() {
        let text = "Jojo was a man who thought he was a loner
        #But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let commands: Vec<String> = Lexer::new(text, &SimpleContext::new()).map(
            |r| r.unwrap().tokens.join("_/_")).collect();

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner");
        assert_eq!(commands[1], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn comment_at_end_of_line_removes_remaining_content() {
        let text = "Jojo was a man # who thought he was a loner
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";

        let commands: Vec<String> = Lexer::new(text, &SimpleContext::new()).map(
            |r| r.unwrap().tokens.join("_/_")).collect();

        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0], "Jojo_/_was_/_a_/_man");
        assert_eq!(commands[1], "But_/_he_/_knew it couldn't_/_last");
        assert_eq!(commands[2], "Jojo left his home_/_in_/_Tuscon, Arizona");
    }

    #[test]
    fn one_position_argument() {
        let text = "$0";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo left his home");

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left his home")
    }

    #[test]
    fn position_argument_outside_quotes_is_error() {
        let text = "Jojo$0";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let commands = Lexer::new(text, &context).next().unwrap();

        assert_eq!(commands, Err(LexerError::new(1, 1, LexerErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn multiple_position_arguments_not_in_quotes_is_an_error() {
        let text = "$0$1";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo");
        context.add_argument(" left his home");

        let commands = Lexer::new(text, &context).next().unwrap();

        assert_eq!(commands, Err(LexerError::new(1, 1, LexerErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn multiple_position_arguments_inside_quotes() {
        let text = "\"$0$1\"";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo");
        context.add_argument(" left his home");

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left his home");
    }

    #[test]
    fn position_argument_can_be_anywhere_in_string_when_inside_quotes() {
        let text = "\"Jojo$0\"";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left his home")
    }

    #[test]
    fn unknown_position_argument_is_error() {
        let text = "$1";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let result = Lexer::new(text, &context).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::UnknownVariable)));
    }

    #[test]
    fn multiple_position_arguments() {
        let text = "\"$0 $1\" $2";
        let mut context = SimpleContext::new();
        context.add_argument("Jojo");
        context.add_argument("left");
        context.add_argument("his home");

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left");
        assert_eq!(commands[0][1], "his home");
    }

    #[test]
    fn curly_brackets_can_be_used_to_separate_position_arguments_from_numbers() {
        let text = "\"${0}345\"";
        let mut context = SimpleContext::new();
        context.add_argument("12");

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "12345");
    }

    #[test]
    fn one_variable() {
        let text = "$foo";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left his home", true);

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left his home")
    }

    #[test]
    fn variable_outside_quotes_is_error() {
        let text = "Jojo$foo";
        let mut context = SimpleContext::new();
        context.set_value("foo", " left his home", true);

        let result = Lexer::new(text, &context).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn variable_can_be_anywhere_in_string_when_inside_quotes() {
        let text = "\"Jojo$foo\"";
        let mut context = SimpleContext::new();
        context.set_value("foo", " left his home", true);

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left his home")
    }

    #[test]
    fn unknown_variable_is_error() {
        let text = "$1";
        let mut context = SimpleContext::new();
        context.add_argument(" left his home");

        let result = Lexer::new(text, &context).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::UnknownVariable)));
    }

    #[test]
    fn multiple_variables() {
        let text = "\"$foo $bar\" $me";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo", true);
        context.set_value("bar", "left", true);
        context.set_value("me", "his home", true);

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left");
        assert_eq!(commands[0][1], "his home");
    }

    #[test]
    fn multiple_variables_not_in_quotes_is_an_error() {
        let text = "$foo$bar";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left", true);
        context.set_value("bar", " his home", true);

        let result = Lexer::new(text, &context).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::InvalidVariableFormat)));
    }

    #[test]
    fn multiple_variables_inside_quotes() {
        let text = "\"$foo$bar\"";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left", true);
        context.set_value("bar", " his home", true);

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left his home");
    }

    #[test]
    fn curly_brackets_can_be_used_to_separate_variables_from_text() {
        let text = "\"${foo}his home\"";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left ", true);

        let commands: Vec<Vec<String>> = Lexer::new(text, &context).map(
            |r| r.unwrap().tokens).collect();

        assert_eq!(commands[0][0], "Jojo left his home");
    }

    #[test]
    fn unterminated_curly_bracket_is_error() {
        let text = "${foo";
        let mut context = SimpleContext::new();
        context.set_value("foo", "Jojo left ", true);

        let result = Lexer::new(text, &context).next().unwrap();

        assert_eq!(result, Err(LexerError::new(1, 1, LexerErrorType::InvalidVariableFormat)));
    }
}