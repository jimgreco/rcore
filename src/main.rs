use std::str::Chars;

struct StatementReader<'a> {
    command_file: Chars<'a>
}

impl<'a> StatementReader<'a> {   
    fn new(file: &'a str) -> StatementReader<'a> {    
        StatementReader { 
            command_file: file.chars()
        }
    }
}

impl<'a> Iterator for StatementReader<'a> {
    type Item = Result<Vec<String>, String>;

    fn next(&mut self) -> Option<Self::Item> {        
        let mut in_quotes = false;
        let mut in_comment = false;
        let mut in_backslash = false;
        let mut word = String::new();
        let mut words: Vec<String> = Vec::new();

        loop {
            let opt = self.command_file.next();
            if opt == None {
                // end of file
                break;
            } else {
                let c = opt.unwrap();

                if c == '\n' {            
                    // continue to the next line if the last character is a backslash
                    if words.len() > 0 && !in_backslash {                        
                        break;
                    }

                    in_comment = false;
                    in_backslash = false;
                } else if !in_comment {
                    if in_backslash {
                        // special characters that are escaped
                        in_backslash = false;
                        
                        if c == '"' || c == '\\' || c == '#' || c == '$' {
                            word.push(c);
                        } else if c == 'n' {
                            word.push('\n');
                        } else {
                            return Some(Err("illegal backslash character".to_string()));
                        }
                    } else if c == '\\' {
                        // escape the next character or continue to the next line
                        in_backslash = true;
                    } else if c == '"' {                    
                        if in_quotes {
                            in_quotes = false;

                            // include zero length words
                            words.push(word.clone());
                            word.clear();
                        } else {
                            // start quotes, includes all whitespace
                            in_quotes = true;                        
                        }
                    } else if c == '#' {
                        in_comment = true;

                        if word.len() > 0 {
                            words.push(word.clone());
                            word.clear();
                        }
                    } else if c.is_whitespace() {
                        if in_quotes {
                            word.push(c);
                        } else {                               
                            if word.len() > 0 {
                                words.push(word.clone());
                                word.clear();
                            }
                        }
                    } else {
                        word.push(c);
                    }
                }
            }
        }

        if in_quotes {
            return Some(Err("quotes not terminated".to_string()));
        }

        // add the last word
        if word.len() > 0 {
            words.push(word.clone());
            word.clear();
        }        

        if words.len() > 0 {
            return Some(Ok(words));
        } else {
            return None;
        }
    }
}


#[test]
fn single_statement_is_processed() {    
    let statement = "context $1";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0], "context_/_$1");
}

#[test]
fn multiple_statements_are_processed() {    
    let statement = "context $1
    create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
    create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
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

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
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

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 3);
    assert_eq!(statements[0], "context_/_$1");
    assert_eq!(statements[1], "create_/_$1_/_com.core.platform.applications.sequencer.Sequencer_/_@$3_/_SEQ01");
    assert_eq!(statements[2], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
}

#[test]
fn whitespace_is_ignored() {    
    let statement = "   context     $1  ";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0], "context_/_$1");
}

#[test]
fn backslash_will_continue_statement_on_the_next_line() {    
    let statement = "   context     $1  \
create $1 com.core.platform.applications.sequencer.Sequencer     @$3     SEQ01
create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3
";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 2);
    assert_eq!(statements[0], "context_/_$1_/_create_/_$1_/_com.core.platform.applications.sequencer.Sequencer_/_@$3_/_SEQ01");
    assert_eq!(statements[1], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3")
}

#[test]
fn quotes_will_mark_a_string() {    
    let statement = "soo    \"foo bar me\"   do";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0], "soo_/_foo bar me_/_do");
}

#[test]
fn backslash_can_represent_special_characters() {    
    let statement = "backslash\\\\ newline\\n pound\\# dollarsign\\$ quotes\\\"";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0], "backslash\\_/_newline\n_/_pound#_/_dollarsign$_/_quotes\"");
}

#[test]
fn comment_at_start_of_line_removes_statement() {    
    let statement = "context $1
    #create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
    create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 2);
    assert_eq!(statements[0], "context_/_$1");
    assert_eq!(statements[1], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
}

#[test]
fn command_at_end_of_line_removes_remaining_content() {    
    let statement = "context # $1
    #create $1 com.core.platform.applications.sequencer.Sequencer @$3 SEQ01
    create $1/handlers/misc com.core.crypto.sequencer.MiscellaneousCommandHandler @$3";

    let statements: Vec<String> = StatementReader::new(statement).map(|r| r.unwrap().join("_/_")).collect();
    assert_eq!(statements.len(), 2);
    assert_eq!(statements[0], "context");
    assert_eq!(statements[1], "create_/_$1/handlers/misc_/_com.core.crypto.sequencer.MiscellaneousCommandHandler_/_@$3");
}