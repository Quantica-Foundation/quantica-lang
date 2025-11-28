// src/lexer/mod.rs
pub mod token;

use token::{Token, TokenWithLocation};

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>,
    start_of_line: bool,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
            start_of_line: true,
        }
    }
    
    pub fn tokenize(&mut self) -> Result<Vec<TokenWithLocation>, String> {
        let mut tokens = Vec::new();
        
        while !self.is_at_end() {
            loop {
                if self.start_of_line {
                    self.handle_indentation(&mut tokens)?;
                }
                self.skip_whitespace_except_newline(); 
    
                if self.current_char().ok() == Some('/') {
                    if self.peek() == Some('/') {
                        // Single-line comment
                        self.advance(); 
                        self.skip_single_line_comment(); 
                        continue; 
                    } else if self.peek() == Some('*') {
                        // Multiline comment /* ... */
                        self.advance(); 
                        self.advance();
                        self.skip_multiline_comment()?;
                        continue;
                    } else {
                        // Division operator
                        break;
                    }
                } else {
                    break; 
                }
            }
    
            if self.is_at_end() {
                break;
            }
    
            if self.current_char().ok() == Some('\n') {
                self.advance();
                tokens.push(self.make_token(Token::Newline, 1));
                continue; 
            }

         
            let _start_col = self.column;
            let ch = self.current_char()?;

           
            let token_result = match ch {
                
                '|' => {
                    if self.is_quantum_ket() {
                        self.ket_notation()
                    } else if self.peek() == Some('>') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::PipeRight, 2))
                    } else if self.peek() == Some('|') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::DoublePipe, 2))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Pipe, 1))
                    }
                }
                '{' => {
                    if self.is_quantum_bra() {
                        self.bra_notation()
                    } else {
                        self.advance(); Ok(self.make_token(Token::LeftBrace, 1))
                    }
                }
                
         
                '+' => { self.advance(); Ok(self.make_token(Token::Plus, 1)) }
                '-' => {
                    if self.peek() == Some('>') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::Arrow, 2))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Minus, 1))
                    }
                }
                '*' => {
                    if self.peek() == Some('*') && self.peek_n(2) == Some('*') {
                        self.advance(); self.advance(); self.advance();
                        Ok(self.make_token(Token::TensorProduct, 3))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Star, 1))
                    }
                }
                '/' => { self.advance(); Ok(self.make_token(Token::Slash, 1)) }
                '%' => { self.advance(); Ok(self.make_token(Token::Percent, 1)) }
                '^' => { self.advance(); Ok(self.make_token(Token::Caret, 1)) }
                '=' => {
                    if self.peek() == Some('=') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::EqualEqual, 2))
                    } else if self.peek() == Some('>') {
                        if self.peek_n(2) == Some('>') {
                            self.advance(); self.advance(); self.advance();
                            Ok(self.make_token(Token::PipeDouble, 3))
                        } else {
                            self.advance(); self.advance();
                            Ok(self.make_token(Token::FatArrow, 2))
                        }
                    } else {
                        self.advance(); Ok(self.make_token(Token::Equal, 1))
                    }
                }
                '!' => {
                    if self.peek() == Some('=') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::NotEqual, 2))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Bang, 1))
                    }
                }
                '<' => {
                    if self.peek() == Some('=') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::LessEqual, 2))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Less, 1))
                    }
                }
                '>' => {
                    if self.peek() == Some('=') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::GreaterEqual, 2))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Greater, 1))
                    }
                }
                '?' => {
                    if self.peek() == Some('.') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::SafeNav, 2))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Question, 1))
                    }
                }
                
              
                '"' => self.string_literal(),
                '\'' => self.char_literal(),
                c if c.is_ascii_digit() => self.number_literal(),
                c if c.is_alphabetic() || c == '_' => Ok(self.identifier_or_keyword()),

                // Punctuation
                '(' => { self.advance(); Ok(self.make_token(Token::LeftParen, 1)) }
                ')' => { self.advance(); Ok(self.make_token(Token::RightParen, 1)) }
                '[' => { self.advance(); Ok(self.make_token(Token::LeftBracket, 1)) }
                ']' => { self.advance(); Ok(self.make_token(Token::RightBracket, 1)) }
                '}' => { self.advance(); Ok(self.make_token(Token::RightBrace, 1)) }
                ',' => { self.advance(); Ok(self.make_token(Token::Comma, 1)) }
                ';' => { self.advance(); Ok(self.make_token(Token::Semicolon, 1)) }
                ':' => {
                    if self.peek() == Some(':') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::DoubleColon, 2))
                    } else if self.peek() == Some('=') {
                        self.advance(); self.advance();
                        Ok(self.make_token(Token::ColonEqual, 2))
                    } else {
                        self.advance(); Ok(self.make_token(Token::Colon, 1))
                    }
                }
                '.' => {
                    if self.peek() == Some('.') {
                        if self.peek_n(2) == Some('=') {
                            self.advance(); self.advance(); self.advance();
                            Ok(self.make_token(Token::RangeInclusive, 3))
                        } else {
                            self.advance(); self.advance();
                            Ok(self.make_token(Token::Range, 2))
                        }
                    } else {
                        self.advance(); Ok(self.make_token(Token::Dot, 1))
                    }
                }
                
                _ => Err(format!("Unexpected character '{}' at line {}, column {}", ch, self.line, self.column)),
            };

            tokens.push(token_result?);
        }
        
   
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            tokens.push(self.make_token(Token::Dedent, 0));
        }
        
        tokens.push(self.make_token(Token::Eof, 0));
        Ok(tokens)
    }

    fn skip_multiline_comment(&mut self) -> Result<(), String> {
        let start_line = self.line;
        
        loop {
            if self.is_at_end() {
                return Err(format!(
                    "Unterminated multiline comment starting at line {}",
                    start_line
                ));
            }
            
            let ch = self.current_char()?;
            
            if ch == '*' && self.peek() == Some('/') {
               
                self.advance();
                self.advance();
                return Ok(());
            }
            
            self.advance();
        }
    }
        
    fn handle_indentation(&mut self, tokens: &mut Vec<TokenWithLocation>) -> Result<(), String> {
    let mut indent_level = 0;
    
  
    while !self.is_at_end() {
        let ch = self.current_char()?;
        match ch {
            ' ' => {
                indent_level += 1;
                self.advance();
            }
            '\t' => {
                indent_level += 4;
                self.advance();
            }
            _ => break, 
        }
    }

 
    if !self.is_at_end() {
        let ch = self.current_char()?;
      
        if ch == '\n' || (ch == '/' && self.peek() == Some('/')) {
           
            self.start_of_line = false;
            return Ok(()); 
        }
    }
    

    self.start_of_line = false;
    
  
    let current_indent = *self.indent_stack.last().unwrap();
    
    if indent_level > current_indent {
        self.indent_stack.push(indent_level);
        tokens.push(self.make_token(Token::Indent, 0));
    } else if indent_level < current_indent {
        while let Some(&stack_level) = self.indent_stack.last() {
            if stack_level <= indent_level {
                break;
            }
            self.indent_stack.pop();
            tokens.push(self.make_token(Token::Dedent, 0));
        }
        
        if self.indent_stack.last() != Some(&indent_level) {
            return Err(format!("Indentation error at line {}", self.line));
        }
    }
    
    Ok(())
}
    
    fn next_token(&mut self) -> Result<Option<TokenWithLocation>, String> {
        let ch = self.current_char()?;
        let start_col = self.column;
        
        let token = match ch {
         
            '#' => {
                self.skip_comment();
                return Ok(None);
            }
            
          

           
            '\n' => {
                let token = Token::Newline;
                
           
                self.advance(); 
                
        
                let length = 1; 
                let token_with_loc = self.make_token(token, length);
                
              
                self.line += 1;
                self.column = 1;

                return Ok(Some(token_with_loc)); 
            }
            
          
            '|' => {
                if self.is_quantum_ket() {
                    return Ok(Some(self.ket_notation()?));
                } else if self.peek() == Some('>') {
                    self.advance();
                    self.advance();
                    Token::PipeRight
                } else if self.peek() == Some('|') {
                    self.advance();
                    self.advance();
                    Token::DoublePipe
                } else {
                    self.advance();
                    Token::Pipe
                }
            }
            
 
            '{' => {
                if self.is_quantum_bra() {
                    return Ok(Some(self.bra_notation()?));
                } else {
                    self.advance();
                    Token::LeftBrace
                }
            }
            
       
            '+' => {
                self.advance();
                Token::Plus
            }
            '-' => {
  
                if self.peek() == Some('>') {
                    self.advance(); 
                    self.advance();
                    Token::Arrow 
                } else {
                    self.advance();
                    Token::Minus 
                }
            }
            '*' => {
           
                if self.peek() == Some('*') && self.input.get(self.position + 2) == Some(&'*') {
                    self.advance(); 
                    self.advance(); 
                    self.advance();
                    Token::TensorProduct
                } 
                
                
                else {
                  
                    self.advance(); 
                    Token::Star
                }
            }
            '/' => {
                self.advance();
                Token::Slash
            }
            '%' => {
                self.advance();
                Token::Percent
            }
            '=' => {
           
                if self.peek() == Some('=') {
                    self.advance(); 
                    self.advance(); 
                    Token::EqualEqual 
                } else if self.peek() == Some('>') {
                
                    if self.input.get(self.position + 2) == Some(&'>') {
                        self.advance(); 
                        self.advance();
                        self.advance(); 
                        Token::PipeDouble 
                    } else {
                        self.advance(); 
                        self.advance(); 
                        Token::FatArrow 
                    }
                } else {
                    self.advance(); 
                    Token::Equal 
                }
            }
            '!' => {
              
                if self.peek() == Some('=') {
                    self.advance(); 
                    self.advance(); 
                    Token::NotEqual
                } else {
                    self.advance();
                    Token::Bang
                }
            }
            '<' => {
                
                if self.peek() == Some('=') {
                    self.advance();
                    self.advance(); 
                    Token::LessEqual
                } else {
                    self.advance(); 
                    Token::Less
                }
            }
            '>' => {
              
                if self.peek() == Some('=') {
                    self.advance();
                    self.advance(); 
                    Token::GreaterEqual
                } else {
                    self.advance();
                    Token::Greater
                }
            }
            '?' => {
        
                if self.peek() == Some('.') {
                    self.advance();
                    self.advance(); .
                    Token::SafeNav
                } else {
                    self.advance(); 
                    Token::Question
                }
            }
            
        
            '"' => return Ok(Some(self.string_literal()?)),
            '\'' => return Ok(Some(self.char_literal()?)),
            
       
            c if c.is_ascii_digit() => return Ok(Some(self.number_literal()?)),
            
         
            c if c.is_alphabetic() || c == '_' => return Ok(Some(self.identifier_or_keyword())),
            
          
            '(' => {
                self.advance();
                Token::LeftParen
            }
            ')' => {
                self.advance();
                Token::RightParen
            }
            '[' => {
                self.advance();
                Token::LeftBracket
            }
            ']' => {
                self.advance();
                Token::RightBracket
            }
            '}' => {
                self.advance();
                Token::RightBrace
            }
            ',' => {
                self.advance();
                Token::Comma
            }
            ';' => {
                self.advance();
                Token::Semicolon
            }
            ':' => {
             
                if self.peek() == Some(':') {
                    self.advance(); 
                    self.advance();
                    Token::DoubleColon
                } else if self.peek() == Some('=') {
                    self.advance();
                    self.advance();
                    Token::ColonEqual
                } else {
                    self.advance(); 
                    Token::Colon
                }
            }
            '.' => {
             
                if self.peek() == Some('.') {
             
                    if self.input.get(self.position + 2) == Some(&'=') {
                        self.advance(); 
                        self.advance(); 
                        self.advance(); 
                        Token::RangeInclusive
                    } else {
                       
                        self.advance(); 
                        self.advance(); 
                        Token::Range
                    }
                } else {
                 
                    self.advance();
                    Token::Dot
                }
            }
            
            _ => return Err(format!("Unexpected character '{}' at line {}, column {}", ch, self.line, self.column)),
        };
        
        let length = self.column - start_col;
        Ok(Some(self.make_token(token, length)))
    }
    
    fn is_quantum_ket(&self) -> bool {
        if self.position + 1 >= self.input.len() {
            return false;
        }
        let next = self.input[self.position + 1];
        next.is_ascii_digit() || next == '+' || next == '-' || next.is_alphabetic()
    }
    
    fn is_quantum_bra(&self) -> bool {
    
        let mut i = self.position + 1;
        while i < self.input.len() {
            let ch = self.input[i];
            if ch == '|' {
                return true;
            }
            if !ch.is_ascii_digit() && ch != '+' && ch != '-' && !ch.is_alphabetic() {
                return false;
            }
            i += 1;
        }
        false
    }
    
    fn ket_notation(&mut self) -> Result<TokenWithLocation, String> {
        let start_col = self.column;
        self.advance(); 
        
        let mut state = String::new();
        while !self.is_at_end() && self.current_char()? != '}' {
            state.push(self.current_char()?);
            self.advance();
        }
        
        if self.is_at_end() || self.current_char()? != '}' {
            return Err(format!("Unclosed quantum ket at line {}", self.line));
        }
        
        self.advance(); 
        let length = self.column - start_col;
        Ok(self.make_token(Token::KetState(state), length))
    }
    
    fn bra_notation(&mut self) -> Result<TokenWithLocation, String> {
        let start_col = self.column;
        self.advance();
        
        let mut state = String::new();
        while !self.is_at_end() && self.current_char()? != '|' {
            state.push(self.current_char()?);
            self.advance();
        }
        
        if self.is_at_end() || self.current_char()? != '|' {
            return Err(format!("Unclosed quantum bra at line {}", self.line));
        }
        
        self.advance(); 
        let length = self.column - start_col;
        Ok(self.make_token(Token::BraState(state), length))
    }
    
    fn identifier_or_keyword(&mut self) -> TokenWithLocation {
        let start = self.position;
        let start_col = self.column;
        
        while !self.is_at_end() {
            let ch = self.current_char().unwrap();
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        
        let text: String = self.input[start..self.position].iter().collect();
        let length = self.column - start_col;
        
        let token = match text.as_str() {
            // Core keywords
            "let" => Token::Let,
            "mut" => Token::Mut,
            "auto" => Token::Auto,
            "func" => Token::Func,
            "circuit" => Token::Circuit,
            "class" => Token::Class,
            "return" => Token::Return,
            "import" => Token::Import,
            "from" => Token::From,
            "use" => Token::Use,
            "as" => Token::As,
            "module" => Token::Module,
            "package" => Token::Package,
            "print"=>Token::Print,
            "echo"=>Token::Echo,
            
            // Control flow
            "if" => Token::If,
            "elif" => Token::Elif,
            "else" => Token::Else,
            "match" => Token::Match,
            "case" => Token::Case,
            "for" => Token::For,
            "in" => Token::In,
            "while" => Token::While,
            "break" => Token::Break,
            "continue" => Token::Continue,
            
            // Error handling
            "Try" => Token::Try,
            "Catch" => Token::Catch,
            "Except" => Token::Except,
            "Finally" => Token::Finally,
            "Throw" => Token::Throw,
            "Safe" => Token::Safe,
            
            // Parallelism
            "parallel" => Token::Parallel,
            "task" => Token::Task,
            "await" => Token::Await,
            "async" => Token::Async,
            "yield" => Token::Yield,
            
            // Logic
            "And" => Token::And,
            "Or" => Token::Or,
            "Not" => Token::Not,
            "Is" => Token::Is,
            
            // Quantum
            "quantum" => Token::Quantum,
            "apply" => Token::Apply,
            "measure" => Token::Measure,
            "Swap"=>Token::Swap,
            "Reset"=>Token::Reset,
            "CZ"=>Token::CZ,
            "CS"=>Token::CS,
            "CT"=>Token::CT,
            "CPhase"=>Token::CPhase,
            "U"=>Token::U,
            "CCX"=>Token::CCX,
            "Toffoli"=>Token::Toffoli,
            "dagger"=>Token::Dagger,
            "controlled"=>Token::Controlled,
            "RX"=>Token::RX,
            "RY"=>Token::RY,
            "RZ"=>Token::RZ,


            //quantum gates
            "Hadamard"=>Token::Hadamard,
            "CNOT"=>Token::Cnot,
            "X"=>Token::X,
            "Y"=>Token::Y,
            "Z"=>Token::Z,
            "S"=>Token::S,
            "T"=>Token::T,
            
            // AI/ML
            "Tensor" => Token::Tensor,
            "Train" => Token::Train,
            "infer" => Token::Infer,
            "Load" => Token::Load,
            "Save" => Token::Save,
            
            // Data structures
            "struct" => Token::Struct,
            "enum" => Token::Enum,
            
            // Modifiers
            "Const" => Token::Const,
            "public" => Token::Public,
            "private" => Token::Private,
            "static" => Token::Static,
            "extern" => Token::Extern,
            "inline" => Token::Inline,
            
            // Meta
            "pragma" => Token::Pragma,
            "sizeof" => Token::Sizeof,
            "typeof" => Token::Typeof,
            
            // Future
            "defer" => Token::Defer,
            "contract" => Token::Contract,
            "where" => Token::Where,
            "generic" => Token::Generic,
            
            // Literals
            "True" => Token::True,
            "False" => Token::False,
            "None" => Token::None,
            "Any" => Token::Any,
            
            // Types
            "Int" => Token::Int,
            "Int8" => Token::Int8,
            "Int16" => Token::Int16,
            "Int32" => Token::Int32,
            "Int64" => Token::Int64,
            "Int128" => Token::Int128,
            "Uint" => Token::Uint,
            "Uint8" => Token::Uint8,
            "Uint16" => Token::Uint16,
            "Uint32" => Token::Uint32,
            "Uint64" => Token::Uint64,
            "Uint128" => Token::Uint128,
            "Float" => Token::Float,
            "Float32" => Token::Float32,
            "Float64" => Token::Float64,
            "Complex" => Token::Complex,
            "Complex64" => Token::Complex64,
            "Complex128" => Token::Complex128,
            "Bool" => Token::Bool,
            "Bit" => Token::Bit,
            "String" => Token::String,
            
            _ => Token::Identifier(text),
        };
        
        self.make_token(token, length)
    }
    
    fn number_literal(&mut self) -> Result<TokenWithLocation, String> {
        let start = self.position;
        let start_col = self.column;
        let mut is_float = false;
        
        // Integer part
        while !self.is_at_end() && self.current_char()?.is_ascii_digit() {
            self.advance();
        }
        
        // Decimal part
        if !self.is_at_end() && self.current_char()? == '.' {
            let next = self.peek();
            if next.is_some() && next.unwrap().is_ascii_digit() {
                is_float = true;
                self.advance(); 
                
                while !self.is_at_end() && self.current_char()?.is_ascii_digit() {
                    self.advance();
                }
            }
        }
        
        // Scientific notation
        if !self.is_at_end() && (self.current_char()? == 'e' || self.current_char()? == 'E') {
            is_float = true;
            self.advance();
            
            if !self.is_at_end() && (self.current_char()? == '+' || self.current_char()? == '-') {
                self.advance();
            }
            
            while !self.is_at_end() && self.current_char()?.is_ascii_digit() {
                self.advance();
            }
        }
        
        let text: String = self.input[start..self.position].iter().collect();
        let length = self.column - start_col;
        
        let token = if is_float {
            Token::FloatLiteral(text.parse().map_err(|_| format!("Invalid float: {}", text))?)
        } else {
            Token::IntLiteral(text.parse().map_err(|_| format!("Invalid integer: {}", text))?)
        };
        
        Ok(self.make_token(token, length))
    }
    
    fn string_literal(&mut self) -> Result<TokenWithLocation, String> {
        let start_col = self.column;
        self.advance(); 
        
        let mut value = String::new();
        
        while !self.is_at_end() && self.current_char()? != '"' {
            if self.current_char()? == '\\' {
                self.advance();
                if self.is_at_end() {
                    return Err("Unterminated string".to_string());
                }
                
                match self.current_char()? {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    _ => return Err(format!("Invalid escape sequence: \\{}", self.current_char()?)),
                }
            } else {
                value.push(self.current_char()?);
            }
            self.advance();
        }
        
        if self.is_at_end() {
            return Err("Unterminated string".to_string());
        }
        
        self.advance(); // 
        let length = self.column - start_col;
        Ok(self.make_token(Token::StringLiteral(value), length))
    }
    
    fn char_literal(&mut self) -> Result<TokenWithLocation, String> {
        let start_col = self.column;
        self.advance(); //
        
        if self.is_at_end() {
            return Err("Empty character literal".to_string());
        }
        
        let ch = self.current_char()?;
        self.advance();
        
        if self.is_at_end() || self.current_char()? != '\'' {
            return Err("Unterminated character literal".to_string());
        }
        
        self.advance(); // 
        let length = self.column - start_col;
        Ok(self.make_token(Token::IntLiteral(ch as i64), length))
    }
    
    // Helper methods
    fn current_char(&self) -> Result<char, String> {
        if self.is_at_end() {
            Err("Unexpected end of input".to_string())
        } else {
            Ok(self.input[self.position])
        }
    }
    
    fn peek(&self) -> Option<char> {
        if self.position + 1 < self.input.len() {
            Some(self.input[self.position + 1])
        } else {
            None
        }
    }

    fn peek_opt(&self) -> Option<char> {
        self.input.get(self.position + 1).cloned()
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.input.get(self.position + n).cloned()
    }

    fn skip_single_line_comment(&mut self) {
        self.advance(); // Consume the second '/'
        
        // Loop while 'current_char()' is Ok and not a newline
        while let Ok(ch) = self.current_char() {
            if ch == '\n' {
                break; // Stop at the newline (don't consume it)
            }
            self.advance();
        }
    }


    fn doc_comment(&mut self) -> Result<Token, String> {
    self.advance(); // Consume the 2nd '/'
    self.advance(); // Consume the 3rd '/'

    let start_line = self.line;
    let mut content = String::new();

    loop {
        match self.current_char() {
            Ok('/') => {
                self.advance(); 
                
                if self.current_char().ok() == Some('/') && self.peek() == Some('/') {
                    self.advance(); // Consume 2nd '/'
                    self.advance(); // Consume 3rd '/'
                    
               
                    let trimmed_content = content.trim();
                    let lines: Vec<&str> = trimmed_content.lines().collect();
                    if lines.is_empty() {
                        return Ok(Token::DocComment(String::new()));
                    }
                    
               
                    let common_indent = lines.iter()
                        .filter(|line| !line.trim().is_empty())
                        .map(|line| line.chars().take_while(|&c| c.is_whitespace()).count())
                        .min()
                        .unwrap_or(0);

                
                    let cleaned_lines: Vec<String> = lines.iter()
                        .map(|line| {
                            if line.len() >= common_indent {
                                line[common_indent..].to_string()
                            } else {
                                line.to_string() 
                            }
                        })
                        .collect();
                    
                    return Ok(Token::DocComment(cleaned_lines.join("\n")));
                }
             
                content.push('/');
            }
            Ok('\n') => {
                self.advance();
                content.push('\n');
            }
            Ok(ch) => {
                self.advance(); 
                content.push(ch);
            }
            Err(_) => {
     
                return Err(format!(
                    "Unterminated documentation comment (///...///) starting at line {}",
                    start_line
                ));
            }
        }
    }
}

    fn advance(&mut self) {
    if !self.is_at_end() {
        let ch = self.input[self.position];
        self.position += 1;
        
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
            self.start_of_line = true;
        } else {
            self.column += 1;
      
        }
    }
}
    
    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
    



fn make_token(&self, token: Token, length: usize) -> TokenWithLocation {
    
   
    let calculated_start_column = self.column.saturating_sub(length);
    
  
    let start_column = if calculated_start_column == 0 {
        1
    } else {
        calculated_start_column
    };

    TokenWithLocation::new(
        token, 
        self.line, 
        start_column, 
        length
    )
}
    
    fn skip_whitespace_except_newline(&mut self) {
        while !self.is_at_end() {
            let ch = self.input[self.position];
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }
    
    fn skip_comment(&mut self) {
        while !self.is_at_end() && self.current_char().unwrap() != '\n' {
            self.advance();
        }
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keywords() {
        let input = "let mut quantum apply measure tensor";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::Let);
        assert_eq!(tokens[1].token, Token::Mut);
        assert_eq!(tokens[2].token, Token::Quantum);
        assert_eq!(tokens[3].token, Token::Apply);
        assert_eq!(tokens[4].token, Token::Measure);
        assert_eq!(tokens[5].token, Token::Tensor);
    }
    
    #[test]
    fn test_quantum_ket() {
        let input = "quantum q = |0}";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::Quantum);
        assert_eq!(tokens[1].token, Token::Identifier("q".to_string()));
        assert_eq!(tokens[2].token, Token::Equal);
        assert_eq!(tokens[3].token, Token::KetState("0".to_string()));
    }
    
    #[test]
    fn test_quantum_states() {
        let input = "|0} |1} |+} |-} |psi}";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::KetState("0".to_string()));
        assert_eq!(tokens[1].token, Token::KetState("1".to_string()));
        assert_eq!(tokens[2].token, Token::KetState("+".to_string()));
        assert_eq!(tokens[3].token, Token::KetState("-".to_string()));
        assert_eq!(tokens[4].token, Token::KetState("psi".to_string()));
    }
    
    #[test]
    fn test_bra_notation() {
        let input = "{0| {1| {psi|";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::BraState("0".to_string()));
        assert_eq!(tokens[1].token, Token::BraState("1".to_string()));
        assert_eq!(tokens[2].token, Token::BraState("psi".to_string()));
    }
    
    #[test]
    fn test_operators() {
        let input = "a + b - c * d / e % f";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[1].token, Token::Plus);
        assert_eq!(tokens[3].token, Token::Minus);
        assert_eq!(tokens[5].token, Token::Star);
        assert_eq!(tokens[7].token, Token::Slash);
        assert_eq!(tokens[9].token, Token::Percent);
    }
    
    #[test]
    fn test_tensor_product() {
        let input = "q1 *** q2";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::Identifier("q1".to_string()));
        assert_eq!(tokens[1].token, Token::TensorProduct);
        assert_eq!(tokens[2].token, Token::Identifier("q2".to_string()));
    }
    
    #[test]
    fn test_star_vs_tensor_product() {
        let input = "a * b *** c";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[1].token, Token::Star);           // single *
        assert_eq!(tokens[3].token, Token::TensorProduct);  // ***
    }
    
    #[test]
    fn test_comparison_operators() {
        let input = "== != < > <= >=";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::EqualEqual);
        assert_eq!(tokens[1].token, Token::NotEqual);
        assert_eq!(tokens[2].token, Token::Less);
        assert_eq!(tokens[3].token, Token::Greater);
        assert_eq!(tokens[4].token, Token::LessEqual);
        assert_eq!(tokens[5].token, Token::GreaterEqual);
    }
    
    #[test]
    fn test_arrows_and_special() {
        let input = "-> => :: .. ..= ?. |> =>> ||";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::Arrow);
        assert_eq!(tokens[1].token, Token::FatArrow);
        assert_eq!(tokens[2].token, Token::DoubleColon);
        assert_eq!(tokens[3].token, Token::Range);
        assert_eq!(tokens[4].token, Token::RangeInclusive);
        assert_eq!(tokens[5].token, Token::SafeNav);
        assert_eq!(tokens[6].token, Token::PipeRight);
        assert_eq!(tokens[7].token, Token::PipeDouble);
        assert_eq!(tokens[8].token, Token::DoublePipe);
    }
    
    #[test]
    fn test_numbers() {
        let input = "42 3.14 2.5e10 1.0e-5";
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        
        assert_eq!(tokens[0].token, Token::IntLiteral(42));
        assert_eq!(tokens[1].token, Token::FloatLiteral(3.14));
        assert!(matches!(tokens[2].token, Token::FloatLiteral(_)));
        assert!(matches!(tokens[3].token, Token::FloatLiteral(_)));
    }

}


