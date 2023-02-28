use std::fmt::{Debug, Display, Formatter};
use crate::command::commands::{AssignmentCommandSpec, DefaultAssignmentCommandSpec, ExecutableCommand, ExecutableCommandSpec};
use crate::command::lexer::{Lexer, LexerContext, LexerError, TokenGroup};

#[derive(Debug, PartialEq)]
pub enum ParserError {
    LexerError(LexerError),
    InvalidVariableName {
        command: TokenGroup,
        variable: String
    },
    UnknownCommand(TokenGroup)
}

pub struct Parser<'a> {
    file: &'a str,
    lexer: Lexer<'a>,
    command_specs: Vec<Box<dyn ExecutableCommandSpec>>,
    error: bool
}

impl<'a> Parser<'a> {
    pub fn new(commands_file: &'a str) -> Parser<'a> {
        let mut parser = Parser {
            file: commands_file,
            lexer: Lexer::new(commands_file),
            command_specs: Vec::new(),
            error: false
        };
        parser.add_command(Box::new(AssignmentCommandSpec::default()));
        parser.add_command(Box::new(DefaultAssignmentCommandSpec::default()));
        parser
    }

    pub fn add_command(&mut self, spec: Box<dyn ExecutableCommandSpec>) {
        self.command_specs.push(spec);
    }


    pub fn next(&mut self, context: &dyn LexerContext)
            -> Option<Result<Box<dyn ExecutableCommand>, ParserError>> {
        if self.error {
            return None;
        }

        match self.lexer.next(context) {
            Some(result) => {
                match result {
                    Ok(mut token_group) => {
                        for spec in &self.command_specs {
                            match spec.validate(&token_group) {
                                Ok(result) => if result {
                                    return Some(Ok(spec.build(&mut token_group)));
                                }
                                Err(e) => {
                                    self.error = true;
                                    return Some(Err(e))
                                }
                            }
                        }

                        self.error = true;
                        return Some(Err(ParserError::UnknownCommand(token_group)))
                    }
                    Err(e) => {
                        self.error = true;
                        Some(Err(ParserError::LexerError(e)))
                    }
                }
            },
            None => None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use crate::command::commands::{AssignmentCommand, DefaultAssignmentCommand, ExecutableCommand, ExecutableCommandSpec, validate_variable};
    use crate::command::lexer::{LexerContext, LexerError, SimpleContext, TokenGroup};
    use crate::command::parser::{Parser, ParserError};
    use crate::command::shell::ShellError;

    #[test]
    fn command_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            foo := soo
            do12 = goo
            ");

        assert_eq!(format!("{:?}", parser.next(&context).unwrap().unwrap()),
                   format!("{:?}", AssignmentCommand {
                       variable: "foo".to_owned(),
                       value: "bar".to_owned(),
                   }));
        assert_eq!(format!("{:?}", parser.next(&context).unwrap().unwrap()),
                   format!("{:?}", DefaultAssignmentCommand {
                       variable: "foo".to_owned(),
                       value: "soo".to_owned(),
                   }));
        assert_eq!(format!("{:?}", parser.next(&context).unwrap().unwrap()),
                   format!("{:?}", AssignmentCommand {
                       variable: "do12".to_owned(),
                       value: "goo".to_owned(),
                   }));
        assert!(parser.next(&context).is_none());
    }

    #[test]
    fn lexer_error_is_passed_through_and_stops_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            fo\"o = soo
            do12 = goo
            ");
        parser.next(&context);

        assert_eq!(ParserError::LexerError(LexerError::UnterminatedQuote { line: 2 }),
                   parser.next(&context).unwrap().err().unwrap());
        assert!(parser.next(&context).is_none());
    }

    #[test]
    fn unknown_command_throws_error_and_stops_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            foo /= soo
            do12 = goo
            ");
        parser.next(&context);

        assert_eq!(ParserError::UnknownCommand(TokenGroup {
            line: 2,
            tokens: vec!["foo".to_owned(), "/=".to_owned(), "soo".to_owned()],
        }), parser.next(&context).unwrap().err().unwrap());
        assert!(parser.next(&context).is_none());
    }

    #[test]
    fn invalid_command_throws_error_and_stops_iteration() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            12foo = soo
            do12 = goo
            ");
        parser.next(&context);

        assert_eq!(ParserError::InvalidVariableName {
            command: TokenGroup {
                line: 2,
                tokens: vec!["12foo".to_owned(), "=".to_owned(), "soo".to_owned()]
            },
            variable: "12foo".to_string(),
        }, parser.next(&context).unwrap().err().unwrap());
        assert!(parser.next(&context).is_none());
    }

    #[derive(Default)]
    struct RemoveVariableCommandSpec {}

    #[derive(Debug)]
    struct RemoveVariableCommand {
        variable: String
    }

    impl ExecutableCommandSpec for RemoveVariableCommandSpec {
        fn validate(&self, command: &TokenGroup) -> Result<bool, ParserError> {
            if command.tokens.len() == 1 {
                let mut chars = command.tokens[0].chars();
                return match chars.nth(0) {
                    None => Ok(false),
                    Some(c) => {
                        let variable: String = chars.skip(1).collect();
                        Ok(c == '!' && validate_variable(&variable))
                    }
                }
            }
            Ok(false)
        }

        fn build(&self, command: &mut TokenGroup) -> Box<dyn ExecutableCommand> {
            Box::new(RemoveVariableCommand {
                variable: command.tokens[0].chars().skip(1).collect()
            })
        }
    }

    impl ExecutableCommand for RemoveVariableCommand {
        fn execute(&self, context: &mut dyn LexerContext)
                -> Result<Option<Box<dyn Any>>, ShellError> {
            Ok(None)
        }
    }

    #[test]
    fn add_a_new_command() {
        let context = SimpleContext::new();
        let mut parser = Parser::new(
            "foo = bar
            !remove_me
            ");
        parser.add_command(Box::new(RemoveVariableCommandSpec::default()));
        parser.next(&context);

        assert_eq!(format!("{:?}", parser.next(&context).unwrap().unwrap()),
                   format!("{:?}", RemoveVariableCommand {
                       variable: "remove_me".to_owned()
                   }));
    }
}