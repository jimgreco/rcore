use std::{fmt, io};
use std::fmt::Formatter;

use thiserror::Error;
use crate::command::context::{UserContext, IoContext};

/// Errors thrown lexing commands from the command file.
#[derive(Debug, Error)]
pub enum LexerError {
    #[error("{src}:{line}:{col}: the command contains an unterminated quote")]
    UnterminatedQuote {
        src: String,
        line: usize,
        col: usize,
    },
    #[error("{src}:{line}:{col}: the escaped character is not in quotes")]
    EscapedCharacterNotInQuotes {
        src: String,
        line: usize,
        col: usize,
    },
    #[error("{src}:{line}:{col}: '{char}' is an invalid escaped character")]
    InvalidEscapedCharacterFormat {
        src: String,
        line: usize,
        col: usize,
        char: String,
    },
    #[error("{src}:{line}:{col}: {var} is an unknown variable")]
    UnknownVariable {
        src: String,
        line: usize,
        col: usize,
        var: String,
    },
    #[error("{src}:{line}:{col}: the variable is in an unknown format")]
    InvalidVariableFormat {
        src: String,
        line: usize,
        col: usize,
    },
    #[error("{src}:{line}:{col}: I/O error: {error}")]
    IoError {
        src: String,
        line: usize,
        col: usize,
        error: io::Error,
    },
}

impl PartialEq for LexerError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LexerError::UnterminatedQuote { src, line, col },
                LexerError::UnterminatedQuote { src: src2, line: line2, col: col2 })
            => src == src2 && line == line2 && col == col2,
            (LexerError::EscapedCharacterNotInQuotes { src, line, col },
                LexerError::EscapedCharacterNotInQuotes { src: src2, line: line2, col: col2 })
            => src == src2 && line == line2 && col == col2,
            (LexerError::InvalidEscapedCharacterFormat { src, line, col, char },
                LexerError::InvalidEscapedCharacterFormat { src: src2, line: line2, col: col2, char: char2 })
            => src == src2 && line == line2 && col == col2 && char == char2,
            (LexerError::UnknownVariable { src, line, col, var },
                LexerError::UnknownVariable { src: src2, line: line2, col: col2, var: var2 })
            => src == src2 && line == line2 && col == col2 && var == var2,
            (LexerError::InvalidVariableFormat { src, line, col },
                LexerError::InvalidVariableFormat { src: src2, line: line2, col: col2 })
            => src == src2 && line == line2 && col == col2,
            (LexerError::IoError { src, line, col, .. },
                LexerError::IoError { src: src2, line: line2, col: col2, .. })
            => src == src2 && line == line2 && col == col2,
            _ => false
        }
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TokenGroup {
    pub line: usize,
    pub tokens: Vec<String>,
}

impl TokenGroup {
    pub fn tokens_string(&self) -> String {
        self.tokens_substring(0, self.tokens.len())
    }

    pub fn tokens_substring(&self, start: usize, end: usize) -> String {
        let mut str = "".to_owned();
        let mut first = true;
        for token in &self.tokens[start..end] {
            if !first {
                str.push(' ');
            }
            first = false;
            str.push('\"');
            str.push_str(token);
            str.push('\"');
        }
        str
    }
}

impl fmt::Display for TokenGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.line)?;
        for token in self.tokens.iter() {
            write!(f, " \"{}\"", token)?;
        }
        write!(f, "")
    }
}

fn expand(context: &UserContext, token: &str, in_quotes: bool, source: &IoContext)
          -> Result<String, LexerError> {
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
                        return Err(invalid_var_format(source));
                    } else if variable.is_empty() {
                        // end of argument
                        match context.get_argument(argument) {
                            None => return Err(unknown_arg(source, argument)),
                            Some(arg) => expanded.push_str(arg)
                        }
                    } else {
                        // end of variable
                        match context.get_value(&variable) {
                            None => return Err(unknown_var(source, &variable)),
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
                                return Err(invalid_var_format(source));
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
                            return Err(invalid_var_format(source));
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
                                return Err(invalid_var_format(source));
                            } else {
                                // finished reading the argument, get the value
                                match context.get_argument(argument) {
                                    None => return Err(unknown_arg(source, argument)),
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
                                return Err(escaped_no_quotes(source));
                            } else {
                                // finished reading the variable, get the value out
                                match context.get_value(&variable) {
                                    None => return Err(unknown_var(source, &variable)),
                                    Some(arg) => expanded.push_str(arg)
                                }
                                variable.clear();
                                did_expansion = true;
                            }
                        }

                        if did_expansion {
                            if has_curly_brackets {
                                if c != '}' {
                                    return Err(invalid_var_format(source));
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
                        return Err(escaped_no_quotes(source));
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

fn unterminated_quote(source: &IoContext) -> LexerError {
    LexerError::UnterminatedQuote {
        src: source.source.to_owned(),
        line: source.line,
        col: source.column,
    }
}

fn invalid_escaped(source: &IoContext, character: u8) -> LexerError {
    let mut str = "\\".to_owned();
    str.push(character as char);
    LexerError::InvalidEscapedCharacterFormat {
        src: source.source.to_owned(),
        line: source.line,
        col: source.column,
        char: str,
    }
}

fn unknown_var(source: &IoContext, variable: &str) -> LexerError {
    LexerError::UnknownVariable {
        src: source.source.to_owned(),
        line: source.line,
        col: source.column,
        var: variable.to_owned(),
    }
}

fn unknown_arg(source: &IoContext, argument: usize) -> LexerError {
    LexerError::UnknownVariable {
        src: source.source.to_owned(),
        line: source.line,
        col: source.column,
        var: argument.to_string(),
    }
}

fn invalid_var_format(source: &IoContext) -> LexerError {
    LexerError::InvalidVariableFormat {
        src: source.source.to_owned(),
        line: source.line,
        col: source.column,
    }
}

fn escaped_no_quotes(source: &IoContext) -> LexerError {
    LexerError::EscapedCharacterNotInQuotes {
        src: source.source.to_owned(),
        line: source.line,
        col: source.column,
    }
}

pub(crate) fn lex_command<'a>(user_context: &UserContext, io_context: &mut IoContext<'a>)
                              -> Option<Result<TokenGroup, LexerError>> {
    let mut in_quotes = false;
    let mut in_comment = false;
    let mut in_backslash = false;
    let mut token = String::new();
    let mut tokens: Vec<String> = Vec::new();
    let mut token_cols = 0;

    io_context.column = 1;
    io_context.line += 1;

    loop {
        match io_context.next_byte() {
            Ok(byte) => {
                match byte {
                    None => {
                        // end of file
                        if in_quotes {
                            return Some(Err(unterminated_quote(io_context)));
                        }

                        // add the last token
                        if !token.is_empty() {
                            tokens.push(match expand(
                                user_context, &token, in_quotes, io_context) {
                                Ok(v) => v,
                                Err(e) => return Some(Err(e))
                            });
                            token.clear();
                            io_context.column += token_cols;
                        }

                        if tokens.len() > 0 {
                            let group = TokenGroup { line: io_context.line, tokens };
                            return Some(Ok(group));
                        } else {
                            return None;
                        }
                    }
                    Some(c) => {
                        token_cols += 1;

                        if c == b'\n' {
                            // end of line
                            if in_quotes {
                                return Some(Err(unterminated_quote(io_context)));
                            }

                            // add the last token
                            if !token.is_empty() {
                                tokens.push(match expand(user_context, &token, in_quotes, io_context) {
                                    Ok(v) => v,
                                    Err(e) => return Some(Err(e))
                                });
                                token.clear();
                                io_context.column += token_cols;
                                token_cols = 0;
                            }

                            // continue to the next line if the last character is a backslash
                            if !in_backslash && tokens.len() > 0 {
                                let group = TokenGroup { line: io_context.line, tokens };
                                return Some(Ok(group));
                            }

                            io_context.line += 1;
                            io_context.column = 0;
                            in_quotes = false;
                            in_comment = false;
                            in_backslash = false;
                        } else if !in_comment {
                            if in_backslash {
                                if !in_quotes {
                                    return Some(Err(invalid_escaped(io_context, c)));
                                }

                                // special characters that are escaped
                                if c == b'n' {
                                    token.push('\n');
                                } else if c == b'\\' {
                                    token.push('\\');
                                } else if c == b'"' {
                                    token.push('"');
                                } else {
                                    return Some(Err(invalid_escaped(io_context, c)));
                                }

                                in_backslash = false;
                            } else if c == b'\\' {
                                // escape the next character or continue to the next line
                                in_backslash = true;
                            } else if c == b'"' {
                                if in_quotes {
                                    // end quotes
                                    // include zero length tokens
                                    tokens.push(match expand(user_context, &token, in_quotes, io_context) {
                                        Ok(v) => v,
                                        Err(e) => return Some(Err(e))
                                    });
                                    token.clear();
                                    io_context.column += token_cols;
                                    token_cols = 0;
                                }
                                in_quotes = !in_quotes
                            } else if c == b'#' && !in_quotes {
                                // start of comment
                                // add the last token
                                if !token.is_empty() {
                                    tokens.push(match expand(user_context, &token, in_quotes, io_context) {
                                        Ok(v) => v,
                                        Err(e) => return Some(Err(e))
                                    });
                                    token.clear();
                                    io_context.column += token_cols;
                                    token_cols = 0;
                                }

                                in_comment = true;
                            } else if c.is_ascii_whitespace() {
                                if in_quotes {
                                    // include all whitespace in quotes
                                    token.push(c as char);
                                } else if !token.is_empty() {
                                    // end the current token
                                    tokens.push(match expand(user_context, &token, in_quotes, io_context) {
                                        Ok(v) => v,
                                        Err(e) => return Some(Err(e))
                                    });
                                    token.clear();
                                    io_context.column += token_cols;
                                    token_cols = 0;
                                }
                                // otherwise, ignore whitespace
                            } else {
                                // add to the current token
                                token.push(c as char);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Some(Err(LexerError::IoError {
                    src: io_context.source.to_owned(),
                    line: io_context.line,
                    col: io_context.column,
                    error: e,
                }));
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use std::io;
    use std::io::{Cursor};
    use crate::command::context::{UserContext, IoContext};
    use crate::command::lexer::{lex_command, LexerError, TokenGroup};

    fn lex_all_commands(context: &UserContext, source: &mut IoContext)
                        -> Result<Vec<TokenGroup>, LexerError> {
        let mut tokens: Vec<TokenGroup> = vec![];
        loop {
            match lex_command(context, source) {
                None => return Ok(tokens),
                Some(result) => tokens.push(result?)
            }
        }
    }

    #[test]
    fn print_debug_command() {
        let text = "create $wtf/bar me
        five = 123
        a \"multiple token\" command
        a $1 multiple line \\
        command
        another";
        let mut context = UserContext::default();
        context.add_argument("testing123");
        context.add_argument("testing456");
        context.set_value("wtf", "foo");
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);

        loop {
            match lex_command(&context, &mut io) {
                Some(result) => println!("{}", result.unwrap()),
                None => return
            }
        }
    }

    #[test]
    fn no_commands_returns_none() {
        let text = "


        ";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let result = lex_command(&context, &mut io);

        assert_eq!(None, result);
    }

    #[test]
    fn quotes_not_terminated_at_end_of_file_throws_error() {
        let text = "foo \"bar me";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let result = lex_command(&context, &mut io).unwrap().err().unwrap();

        assert_eq!(LexerError::UnterminatedQuote {
            src: "test".to_owned(),
            line: 1,
            col: 5,
        }, result);
    }

    #[test]
    fn quotes_not_terminated_at_end_of_line_throws_error() {
        let text = "foo \"bar me
        hey there";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let result = lex_command(&context, &mut io).unwrap().err().unwrap();

        assert_eq!(LexerError::UnterminatedQuote {
            src: "test".to_owned(),
            line: 1,
            col: 5,
        }, result);
    }

    #[test]
    fn invalid_escaped_character_throws_error() {
        let text = "foo \"b\\^br\" me";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let result = lex_command(&context, &mut io).unwrap().err().unwrap();

        assert_eq!(LexerError::InvalidEscapedCharacterFormat {
            src: "test".to_owned(),
            line: 1,
            col: 5,
            char: "\\^".to_owned(),
        }, result);
    }

    #[test]
    fn escaped_characters() {
        let text = "foo \"bar \\n me \\\" now \\\\ abc \"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands = lex_command(&context, &mut io).unwrap();

        assert_eq!("bar \n me \" now \\ abc ", commands.unwrap().tokens[1]);
    }

    #[test]
    fn single_command_is_processed() {
        let text = "foo bar";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<String> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.join("_/_")).collect();

        assert_eq!(1, commands.len());
        assert_eq!("foo_/_bar", commands[0]);
    }

    #[test]
    fn multiple_commands_are_processed() {
        let text = "Jojo was a man who thought he was a loner
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<String> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.join("_/_")).collect();

        assert_eq!(3, commands.len());
        assert_eq!("Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner", commands[0]);
        assert_eq!("But_/_he_/_knew it couldn't_/_last", commands[1]);
        assert_eq!("Jojo left his home_/_in_/_Tuscon, Arizona", commands[2]);
    }

    #[test]
    fn empty_commands_are_ignored() {
        let text = "Jojo was a man who thought he was a loner

        But he \"knew it couldn't\" last

        \"Jojo left his home\" in \"Tuscon, Arizona\"

        ";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<String> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.join("_/_")).collect();

        assert_eq!(3, commands.len());
        assert_eq!("Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner", commands[0]);
        assert_eq!("But_/_he_/_knew it couldn't_/_last", commands[1]);
        assert_eq!("Jojo left his home_/_in_/_Tuscon, Arizona", commands[2]);
    }

    #[test]
    fn whitespace_is_ignored() {
        let text = "   foo     bar  ";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<String> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.join("_/_")).collect();

        assert_eq!(1, commands.len());
        assert_eq!("foo_/_bar", commands[0]);
    }

    #[test]
    fn backslash_will_continue_command_on_the_next_line() {
        let text = "Jojo was a man who thought he was a loner \
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<String> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.join("_/_")).collect();

        assert_eq!(2, commands.len());
        assert_eq!("Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner_/_But_/_he_/_knew it couldn't_/_last",
                   commands[0]);
        assert_eq!("Jojo left his home_/_in_/_Tuscon, Arizona", commands[1]);
    }

    #[test]
    fn empty_quotes_can_be_a_token() {
        let text = "foo \"\" bar";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        let tokens = &commands[0];
        assert_eq!(3, tokens.len());
        assert_eq!("foo", tokens[0]);
        assert_eq!("", tokens[1]);
        assert_eq!("bar", tokens[2]);
    }

    #[test]
    fn backslash_not_in_quotes_is_error() {
        let text = "back\\\"slash";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let result = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::InvalidEscapedCharacterFormat {
            src: "test".to_owned(),
            line: 1,
            col: 1,
            char: "\\\"".to_owned(),
        }, result);
    }

    #[test]
    fn comment_at_start_of_line_removes_command() {
        let text = "Jojo was a man who thought he was a loner
        #But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<String> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.join("_/_")).collect();

        assert_eq!(2, commands.len());
        assert_eq!("Jojo_/_was_/_a_/_man_/_who_/_thought_/_he_/_was_/_a_/_loner", commands[0]);
        assert_eq!("Jojo left his home_/_in_/_Tuscon, Arizona", commands[1]);
    }

    #[test]
    fn comment_at_end_of_line_removes_remaining_content() {
        let text = "Jojo was a man # who thought he was a loner
        But he \"knew it couldn't\" last
        \"Jojo left his home\" in \"Tuscon, Arizona\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let context = UserContext::default();

        let commands: Vec<String> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.join("_/_")).collect();

        assert_eq!(3, commands.len());
        assert_eq!("Jojo_/_was_/_a_/_man", commands[0]);
        assert_eq!("But_/_he_/_knew it couldn't_/_last", commands[1]);
        assert_eq!("Jojo left his home_/_in_/_Tuscon, Arizona", commands[2]);
    }

    #[test]
    fn one_position_argument() {
        let text = "$0";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument("Jojo left his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left his home", commands[0][0])
    }

    #[test]
    fn position_argument_outside_quotes_is_error() {
        let text = "Jojo$0";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument(" left his home");

        let commands = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::EscapedCharacterNotInQuotes {
            src: "test".to_owned(),
            line: 1,
            col: 1,
        }, commands);
    }

    #[test]
    fn multiple_position_arguments_not_in_quotes_is_an_error() {
        let text = "$0$1";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument("Jojo");
        context.add_argument(" left his home");

        let commands = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::InvalidVariableFormat {
            src: "test".to_owned(),
            line: 1,
            col: 1,
        }, commands);
    }

    #[test]
    fn multiple_position_arguments_inside_quotes() {
        let text = "\"$0$1\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument("Jojo");
        context.add_argument(" left his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left his home", commands[0][0]);
    }

    #[test]
    fn position_argument_can_be_anywhere_in_string_when_inside_quotes() {
        let text = "\"Jojo$0\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument(" left his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left his home", commands[0][0])
    }

    #[test]
    fn unknown_position_argument_is_error() {
        let text = "$1";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument(" left his home");

        let result = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::UnknownVariable {
            src: "test".to_owned(),
            line: 1,
            col: 1,
            var: "1".to_owned(),
        }, result);
    }

    #[test]
    fn multiple_position_arguments() {
        let text = "\"$0 $1\" $2";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument("Jojo");
        context.add_argument("left");
        context.add_argument("his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left", commands[0][0]);
        assert_eq!("his home", commands[0][1]);
    }

    #[test]
    fn curly_brackets_can_be_used_to_separate_position_arguments_from_numbers() {
        let text = "\"${0}345\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument("12");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("12345", commands[0][0]);
    }

    #[test]
    fn one_variable() {
        let text = "$foo";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", "Jojo left his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left his home", commands[0][0])
    }

    #[test]
    fn variable_outside_quotes_is_error() {
        let text = "Jojo$foo";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", " left his home");

        let result = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::EscapedCharacterNotInQuotes {
            src: "test".to_owned(),
            line: 1,
            col: 1,
        }, result);
    }

    #[test]
    fn variable_can_be_anywhere_in_string_when_inside_quotes() {
        let text = "\"Jojo$foo\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", " left his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left his home", commands[0][0])
    }

    #[test]
    fn unknown_variable_is_error() {
        let text = "$1";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.add_argument(" left his home");

        let result = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::UnknownVariable {
            src: "test".to_owned(),
            line: 1,
            col: 1,
            var: "1".to_owned(),
        }, result);
    }

    #[test]
    fn multiple_variables() {
        let text = "\"$foo $bar\" $me";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", "Jojo");
        context.set_value("bar", "left");
        context.set_value("me", "his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left", commands[0][0]);
        assert_eq!("his home", commands[0][1]);
    }

    #[test]
    fn multiple_variables_not_in_quotes_is_an_error() {
        let text = "$foo$bar";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", "Jojo left");
        context.set_value("bar", " his home");

        let result = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::EscapedCharacterNotInQuotes {
            src: "test".to_owned(),
            line: 1,
            col: 1,
        }, result);
    }

    #[test]
    fn multiple_variables_inside_quotes() {
        let text = "\"$foo$bar\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", "Jojo left");
        context.set_value("bar", " his home");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left his home", commands[0][0]);
    }

    #[test]
    fn curly_brackets_can_be_used_to_separate_variables_from_text() {
        let text = "\"${foo}his home\"";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", "Jojo left ");

        let commands: Vec<Vec<String>> = lex_all_commands(&context, &mut io)
            .unwrap().iter().map(|r| r.tokens.clone()).collect();

        assert_eq!("Jojo left his home", commands[0][0]);
    }

    #[test]
    fn unterminated_curly_bracket_is_error() {
        let text = "${foo";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("foo", "Jojo left ");

        let result = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::InvalidVariableFormat {
            src: "test".to_owned(),
            line: 1,
            col: 1,
        }, result);
    }

    #[test]
    fn invalid_character_in_first_part_of_variable_is_error() {
        let text = "$@foo";
        let mut cursor = Cursor::new(text.as_bytes());
        let mut sink = io::sink();
        let mut io = IoContext::new("test", &mut cursor, &mut sink);
        let mut context = UserContext::default();
        context.set_value("f@oo", "Jojo left ");

        let result = lex_command(&context, &mut io)
            .unwrap().err().unwrap();

        assert_eq!(LexerError::InvalidVariableFormat {
            src: "test".to_owned(),
            line: 1,
            col: 1,
        }, result);
    }
}