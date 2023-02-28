use std::fmt::Debug;
use crate::command::commands::ExecutableCommand;
use crate::command::lexer::{lex_command, LexerError, TokenGroup};
use crate::command::context::{Context, Source};

use thiserror::Error;

/// Errors thrown parsing commands in the command file.
#[derive(Debug, PartialEq, Error)]
pub enum ParserError {
    #[error(transparent)]
    LexerError(LexerError),
    #[error("invalid variable name: {variable}, command={command}")]
    InvalidVariableName {
        command: TokenGroup,
        variable: String
    },
    #[error("unknown command: {0}")]
    UnknownCommand(TokenGroup)
}

pub(crate) fn parse_command(context: &Context, source: &mut Source)
        -> Option<Result<Box<dyn ExecutableCommand>, ParserError>> {
    match lex_command(context, source) {
        Some(result) => {
            match result {
                Ok(mut token_group) => {
                    for spec in context.get_command_specs() {
                        match spec.validate(&token_group) {
                            Ok(result) => if result {
                                return Some(Ok(spec.build(&mut token_group)));
                            }
                            Err(e) => return Some(Err(e))
                        }
                    }
                    return Some(Err(ParserError::UnknownCommand(token_group)))
                }
                Err(e) => Some(Err(ParserError::LexerError(e)))
            }
        }
        None => None
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::io;
    use crate::command::commands::{AssignmentCommand, AssignmentCommandSpec,
                                   DefaultAssignmentCommand, DefaultAssignmentCommandSpec,
                                   ExecutableCommand, ExecutableCommandSpec, validate_variable};
    use crate::command::context::{Context, Source};
    use crate::command::lexer::{LexerError, TokenGroup};
    use crate::command::parser::{parse_command, ParserError};
    use crate::command::shell::ShellError;

    fn new_context() -> Context {
        let mut context = Context::default();
        context.add_command_spec(Box::new(AssignmentCommandSpec {}));
        context.add_command_spec(Box::new(DefaultAssignmentCommandSpec {}));
        context
    }

    #[test]
    fn command_iteration() {
        let context = new_context();
        let text = "foo = bar\nfoo := soo\ndo12 = goo";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::sink();
        let mut source = Source::new_test(&mut cursor, &mut sink);

        assert_eq!(format!("{:?}", parse_command(&context, &mut source).unwrap().unwrap()),
                   format!("{:?}", AssignmentCommand {
                       variable: "foo".to_owned(),
                       value: "bar".to_owned(),
                   }));
        assert_eq!(format!("{:?}", parse_command(&context, &mut source).unwrap().unwrap()),
                   format!("{:?}", DefaultAssignmentCommand {
                       variable: "foo".to_owned(),
                       value: "soo".to_owned(),
                   }));
        assert_eq!(format!("{:?}", parse_command(&context, &mut source).unwrap().unwrap()),
                   format!("{:?}", AssignmentCommand {
                       variable: "do12".to_owned(),
                       value: "goo".to_owned(),
                   }));
        assert!(parse_command(&context, &mut source).is_none());
    }

    #[test]
    fn lexer_error_is_passed_through() {
        let context = new_context();
        let text = "foo = bar
foo = s\"oo
do12 = goo
            ";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::sink();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        parse_command(&context, &mut source);

        assert_eq!(ParserError::LexerError(LexerError::UnterminatedQuote {
            src: "test".to_owned(), line: 2, col: 7,
        }), parse_command(&context, &mut source).unwrap().err().unwrap());
    }

    #[test]
    fn unknown_command_throws_error() {
        let context = new_context();
        let text = "foo = bar
            foo /= soo
            do12 = goo
            ";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::sink();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        parse_command(&context, &mut source);

        assert_eq!(ParserError::UnknownCommand(TokenGroup {
            line: 2,
            tokens: vec!["foo".to_owned(), "/=".to_owned(), "soo".to_owned()],
        }), parse_command(&context, &mut source).unwrap().err().unwrap());
    }

    #[test]
    fn invalid_command_throws_error() {
        let context = new_context();
        let text = "foo = bar
            12foo = soo
            do12 = goo
            ";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::sink();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        parse_command(&context, &mut source);

        assert_eq!(ParserError::InvalidVariableName {
            command: TokenGroup {
                line: 2,
                tokens: vec!["12foo".to_owned(), "=".to_owned(), "soo".to_owned()]
            },
            variable: "12foo".to_string(),
        }, parse_command(&context, &mut source).unwrap().err().unwrap());
    }

    #[derive(Default)]
    struct RemoveVariableCommandSpec {}

    #[derive(Debug)]
    struct RemoveVariableCommand {
        #[warn(dead_code)]
        var: String
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
                var: command.tokens[0].chars().skip(1).collect()
            })
        }
    }

    impl ExecutableCommand for RemoveVariableCommand {
        fn execute(&self, _context: &mut Context) -> Result<Option<Box<dyn Any>>, ShellError> {
            Ok(None)
        }
    }

    #[test]
    fn add_a_new_command() {
        let mut context = new_context();
        let text = "foo = bar
            !remove_me
            ";
        let mut cursor = Source::cursor(text);
        let mut sink = Source::sink();
        let mut source = Source::new_test(&mut cursor, &mut sink);
        context.add_command_spec(Box::new(RemoveVariableCommandSpec::default()));
        parse_command(&context, &mut source);

        assert_eq!(format!("{:?}", parse_command(&context, &mut source).unwrap().unwrap()),
                   format!("{:?}", RemoveVariableCommand {
                       var: "remove_me".to_owned()
                   }));
    }
}