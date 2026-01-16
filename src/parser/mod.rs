/* src/parser/mod.rs */
pub mod ast;

use crate::lexer::token::{Token, TokenWithLocation};
use crate::parser::ast::UnaryOperator;
use ast::*;

pub struct Parser {
    tokens: Vec<TokenWithLocation>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<TokenWithLocation>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn debug_current_context(&self, msg: &str) {
        if self.position < self.tokens.len() {
            eprintln!("DEBUG {}: pos={}, token={:?}, line={}, col={}",
                msg,
                self.position,
                self.tokens[self.position].token,
                self.tokens[self.position].line,
                self.tokens[self.position].column
            );

            // Show next 5 tokens
            for i in 0..5 {
                if self.position + i < self.tokens.len() {
                    eprintln!("  [+{}] {:?}", i, self.tokens[self.position + i].token);
                }
            }
        }
    }

    pub fn parse(&mut self) -> Result<ASTNode, String> {
        let mut statements = Vec::new();

        while !self.is_at_end() {
            if self.match_token(&Token::Newline) {
                continue;
            }

            if self.match_token(&Token::Dedent) {
                continue;
            }

            statements.push(self.parse_statement()?);
        }

        Ok(ASTNode::Program(statements))
    }
    fn parse_array_literal(&mut self) -> Result<ASTNode, String> {
        let mut elements = Vec::new();

        if self.check(&Token::RightBracket) {
            self.advance();
            return Ok(ASTNode::ArrayLiteral(elements));
        }

        loop {
            elements.push(self.parse_expression()?);

            if self.check(&Token::RightBracket) {
                break;
            }

            self.expect(&Token::Comma)?;
        }

        self.expect(&Token::RightBracket)?;

        Ok(ASTNode::ArrayLiteral(elements))
    }

    fn parse_statement(&mut self) -> Result<ASTNode, String> {
        self.skip_newlines();

        if self.is_at_end() {
            return Err("Unexpected end of file while parsing statement".to_string());
        }

        if self.check(&Token::Dedent) {
            return Err(format!(
                "Unexpected Dedent token at line {}. This might indicate mismatched indentation.",
                self.current()?.line
            ));
        }

        match &self.current()?.token {
            Token::Let => self.parse_let_declaration(),
            Token::Mut => self.parse_mut_declaration(),
            Token::Try => self.parse_try_catch(),
            Token::Quantum => self.parse_quantum_declaration(),
            Token::Import => self.parse_import_statement(),
            Token::From => self.parse_from_import(),
            Token::Func => self.parse_function_declaration(),
            Token::Circuit => self.parse_circuit_declaration(),
            Token::Return => self.parse_return(),
            Token::If => self.parse_if(),
            Token::Match => self.parse_match(),
            Token::For => self.parse_for(),
            Token::Apply => self.parse_apply_statement(),
            Token::While => self.parse_while(),
            Token::Class => self.parse_class_declaration(),
            Token::New => self.parse_new_instance(),
            Token::Break => {
                self.advance();
                self.skip_newlines();
                Ok(ASTNode::Break)
            }
            Token::Continue => {
                self.advance();
                self.skip_newlines();
                Ok(ASTNode::Continue)
            }
            _ => {
                let expr = self.parse_expression()?;

                if self.match_token(&Token::Equal) {
                    let value = self.parse_expression()?;
                    self.skip_newlines();
                    return Ok(ASTNode::Assignment {
                        target: Box::new(expr),
                        value: Box::new(value),
                    });
                }

                self.skip_newlines();
                Ok(expr)
            }
        }
    }

    fn parse_gate_expression(&mut self) -> Result<ASTNode, String> {
        let loc = self.get_loc(self.current()?);

        if self.match_token(&Token::Controlled) {
            self.expect(&Token::LeftParen)?;
            let gate_expr = self.parse_gate_expression()?;
            self.expect(&Token::RightParen)?;
            Ok(ASTNode::Controlled {
                gate_expr: Box::new(gate_expr),
                loc,
            })
        } else if self.match_token(&Token::Dagger) {
            self.expect(&Token::LeftParen)?;
            let gate_expr = self.parse_gate_expression()?;
            self.expect(&Token::RightParen)?;
            Ok(ASTNode::Dagger {
                gate_expr: Box::new(gate_expr),
                loc,
            })
        } else {
            let gate_token = self.expect_identifier()?;
            let gate_name = self.extract_identifier_name(&gate_token)?;

            if self.check(&Token::LeftParen) {
                self.advance();
                let mut params = Vec::new();
                if !self.check(&Token::RightParen) {
                    loop {
                        params.push(self.parse_expression()?);
                        if !self.match_token(&Token::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&Token::RightParen)?;
                Ok(ASTNode::ParameterizedGate {
                    name: gate_name,
                    parameters: params,
                    loc: self.get_loc(&gate_token),
                })
            } else {
                Ok(ASTNode::Gate {
                    name: gate_name,
                    loc: self.get_loc(&gate_token),
                })
            }
        }
    }

    fn parse_apply_statement(&mut self) -> Result<ASTNode, String> {
        let loc = self.get_loc(self.current()?);
        self.expect(&Token::Apply)?;

        if self.check(&Token::Dagger) {
            let saved_pos = self.position;
            self.advance();

            if self.check(&Token::LeftParen) {
                self.advance();

                let is_gate_expr = matches!(
                    self.current().ok().map(|t| &t.token),
                    Some(Token::X)
                        | Some(Token::Y)
                        | Some(Token::Z)
                        | Some(Token::S)
                        | Some(Token::T)
                        | Some(Token::Hadamard)
                        | Some(Token::Cnot)
                        | Some(Token::Swap)
                        | Some(Token::Reset)
                        | Some(Token::CZ)
                        | Some(Token::CS)
                        | Some(Token::CT)
                        | Some(Token::CCX)
                        | Some(Token::Toffoli)
                        | Some(Token::RX)
                        | Some(Token::RY)
                        | Some(Token::RZ)
                        | Some(Token::U)
                        | Some(Token::CPhase)
                        | Some(Token::Controlled)
                        | Some(Token::Dagger)
                );

                self.position = saved_pos;

                if is_gate_expr {
                } else {
                    self.advance();
                    self.expect(&Token::LeftParen)?;
                    let callee_expr = self.parse_expression()?;
                    self.expect(&Token::RightParen)?;
                    self.expect(&Token::LeftParen)?;
                    let arguments = self.parse_arguments()?;
                    self.skip_newlines();

                    return Ok(ASTNode::FunctionCall {
                        callee: Box::new(callee_expr),
                        arguments,
                        loc,
                        is_dagger: true,
                    });
                }
            } else {
                self.position = saved_pos;
            }
        }

        let saved_pos = self.position;
        let mut is_function_call = false;

        if self.check(&Token::Identifier("".to_string())) {
            let mut temp_pos = self.position;
            temp_pos += 1;

            while temp_pos < self.tokens.len() {
                if std::mem::discriminant(&self.tokens[temp_pos].token)
                    == std::mem::discriminant(&Token::Dot)
                {
                    temp_pos += 1;
                    if temp_pos < self.tokens.len()
                        && std::mem::discriminant(&self.tokens[temp_pos].token)
                            == std::mem::discriminant(&Token::Identifier("".to_string()))
                    {
                        temp_pos += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            if temp_pos < self.tokens.len()
                && std::mem::discriminant(&self.tokens[temp_pos].token)
                    == std::mem::discriminant(&Token::LeftParen)
            {
                is_function_call = true;
            }
        }

        self.position = saved_pos;

        if is_function_call {
            let mut callee_expr = self.parse_primary()?;

            while self.check(&Token::Dot) {
                self.advance();
                let name_loc = self.expect_identifier()?;
                let member = self.extract_identifier_name(&name_loc)?;

                callee_expr = ASTNode::MemberAccess {
                    object: Box::new(callee_expr),
                    member,
                };
            }

            self.expect(&Token::LeftParen)?;
            let arguments = self.parse_arguments()?;
            self.skip_newlines();

            return Ok(ASTNode::FunctionCall {
                callee: Box::new(callee_expr),
                arguments,
                loc,
                is_dagger: false,
            });
        }

        let apply_node = self.parse_gate_application(loc)?;
        self.skip_newlines();
        Ok(apply_node)
    }

    fn parse_gate_application(&mut self, loc: Loc) -> Result<ASTNode, String> {
        let is_simple_gate = matches!(
            self.current()?.token,
            Token::X
                | Token::Y
                | Token::Z
                | Token::Hadamard
                | Token::S
                | Token::T
                | Token::Cnot
                | Token::Swap
                | Token::Reset
                | Token::CZ
                | Token::CS
                | Token::CT
                | Token::CCX
                | Token::Toffoli
        );

        let is_parameterized_gate = matches!(
            self.current()?.token,
            Token::RX | Token::RY | Token::RZ | Token::U | Token::CPhase
        );

        if is_simple_gate || is_parameterized_gate {
            let gate_token = self.expect_identifier()?;
            let gate_name = self.extract_identifier_name(&gate_token)?;

            if is_parameterized_gate {
                self.expect(&Token::LeftParen)?;
                let mut params = Vec::new();
                if !self.check(&Token::RightParen) {
                    loop {
                        params.push(self.parse_expression()?);
                        if !self.match_token(&Token::Comma) {
                            break;
                        }
                    }
                }
                self.expect(&Token::RightParen)?;

                let gate_expr = ASTNode::ParameterizedGate {
                    name: gate_name,
                    parameters: params,
                    loc,
                };

                self.skip_newlines();
                self.expect(&Token::LeftParen)?;
                let arguments = self.parse_arguments()?;

                return Ok(ASTNode::Apply {
                    gate_expr: Box::new(gate_expr),
                    arguments,
                    loc,
                });
            } else {
                let gate_expr = ASTNode::Gate {
                    name: gate_name,
                    loc,
                };

                self.skip_newlines();
                self.expect(&Token::LeftParen)?;
                let arguments = self.parse_arguments()?;

                return Ok(ASTNode::Apply {
                    gate_expr: Box::new(gate_expr),
                    arguments,
                    loc,
                });
            }
        }

        let gate_expr = self.parse_gate_expression()?;

        self.skip_newlines();

        if !self.check(&Token::LeftParen) {
            let current_token = self.current()?;
            return Err(format!(
                "Expected '(' for qubit arguments after gate '{}' at line {}, column {}, but found {:?}",
                match &gate_expr {
                    ASTNode::Gate { name, .. } => name.as_str(),
                    ASTNode::ParameterizedGate { name, .. } => name.as_str(),
                    ASTNode::Controlled { .. } => "controlled",
                    ASTNode::Dagger { .. } => "dagger",
                    _ => "gate"
                },
                current_token.line, current_token.column, current_token.token
            ));
        }
        self.advance();
        let arguments = self.parse_arguments()?;

        Ok(ASTNode::Apply {
            gate_expr: Box::new(gate_expr),
            arguments,
            loc,
        })
    }

    fn parse_let_declaration(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Let)?;

        let name_loc = self.expect_identifier()?;
        let name = self.extract_identifier_name(&name_loc)?;

        let type_annotation = if self.match_token(&Token::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let value_box = if self.match_token(&Token::Equal) {
            Box::new(self.parse_expression()?)
        } else {
            Box::new(ASTNode::NoneLiteral)
        };

        self.skip_newlines();

        Ok(ASTNode::LetDeclaration {
            name,
            type_annotation,
            value: value_box,
            is_mutable: false,
        })
    }

    fn parse_mut_declaration(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Mut)?;

        let name_loc = self.expect_identifier()?;
        let name = self.extract_identifier_name(&name_loc)?;

        let type_annotation = if self.match_token(&Token::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let value_box = if self.match_token(&Token::Equal) {
            Box::new(self.parse_expression()?)
        } else {
            Box::new(ASTNode::NoneLiteral)
        };

        self.skip_newlines();

        Ok(ASTNode::LetDeclaration {
            name,
            type_annotation,
            value: value_box,
            is_mutable: true,
        })
    }

    fn parse_try_catch(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Try)?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();

        let try_block = self.parse_block()?;

        self.expect(&Token::Catch)?;

        let error_variable = if self.check(&Token::Colon) {
            None
        } else {
            let name_loc = self.expect_identifier()?;
            let name = self.extract_identifier_name(&name_loc)?;
            Some(name)
        };

        self.expect(&Token::Colon)?;
        self.skip_newlines();

        let catch_block = self.parse_block()?;

        Ok(ASTNode::TryCatch {
            try_block: Box::new(try_block),
            error_variable,
            catch_block: Box::new(catch_block),
        })
    }

    fn parse_import_statement(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Import)?;

        let path;
        if self.check(&Token::StringLiteral("".into())) {
            let current_token = self.current()?.clone();
            let path_str = match &current_token.token {
                Token::StringLiteral(s) => {
                    self.advance();
                    s.clone()
                }
                _ => unreachable!(),
            };
            path = ImportPath::File(path_str);
        } else {
            path = ImportPath::Module(self.parse_module_path()?);
        }

        self.expect(&Token::As)?;

        let name_loc = self.expect_identifier()?;
        let alias = self.extract_identifier_name(&name_loc)?;

        self.skip_newlines();

        Ok(ASTNode::Import { path, alias })
    }

    fn parse_module_path(&mut self) -> Result<Vec<String>, String> {
        let mut path_segments = Vec::new();

        let name_loc = self.expect_identifier()?;
        let name = self.extract_identifier_name(&name_loc)?;
        path_segments.push(name);

        while self.match_token(&Token::Dot) {
            let name_loc = self.expect_identifier()?;
            let name = self.extract_identifier_name(&name_loc)?;
            path_segments.push(name);
        }

        Ok(path_segments)
    }

    fn parse_quantum_declaration(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Quantum)?;

        let name_loc = self.expect_identifier()?;
        let name = self.extract_identifier_name(&name_loc)?;

        let size = if self.match_token(&Token::LeftBracket) {
            let size_expr = self.parse_expression()?;
            self.expect(&Token::RightBracket)?;
            Some(Box::new(size_expr))
        } else {
            None
        };

        let initial_state = if self.match_token(&Token::Equal) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        if size.is_some() && initial_state.is_some() {
            return Err(format!(
                "Syntax Error at {}: Cannot define a register size `[N]` and an initial state `=` at the same time.",
                self.get_loc(&name_loc)
            ));
        }

        self.skip_newlines();

        Ok(ASTNode::QuantumDeclaration {
            name,
            size,
            initial_state,
        })
    }

    fn parse_from_import(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::From)?;

        let path;
        if self.check(&Token::StringLiteral("".into())) {
            let current_token = self.current()?.clone();
            let path_str = match &current_token.token {
                Token::StringLiteral(s) => {
                    self.advance();
                    s.clone()
                }
                _ => unreachable!(),
            };
            path = ImportPath::File(path_str);
        } else {
            path = ImportPath::Module(self.parse_module_path()?);
        }

        self.expect(&Token::Import)?;

        let spec;
        if self.match_token(&Token::Star) {
            spec = ImportSpec::All;
        } else {
            let mut names = Vec::new();
            loop {
                let name_loc = self.expect_identifier()?;
                let name = self.extract_identifier_name(&name_loc)?;
                names.push(name);

                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
            spec = ImportSpec::List(names);
        }

        self.skip_newlines();

        Ok(ASTNode::FromImport { path, spec })
    }

    fn parse_function_declaration(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Func)?;

        let name_loc = self.expect_identifier()?;
        let name = self.extract_identifier_name(&name_loc)?;

        self.expect(&Token::LeftParen)?;
        let parameters = self.parse_parameters()?;

        self.skip_newlines();
        self.expect(&Token::RightParen)?;

        self.skip_layout_tokens();

        let return_type = if self.match_token(&Token::Arrow) {
            self.skip_newlines();
            Some(self.parse_type()?)
        } else {
            None
        };

        if self.check(&Token::Colon) {
            self.advance();
        }
        self.skip_newlines();

        let body = self.parse_block()?;

        Ok(ASTNode::FunctionDeclaration {
            name,
            parameters,
            return_type,
            body: Box::new(body),
        })
    }

    fn parse_class_declaration(&mut self) -> Result<ASTNode, String> {
        let loc = self.get_loc(self.current()?);
        self.expect(&Token::Class)?;

        let name_loc = self.expect_identifier()?;
        let name = self.extract_identifier_name(&name_loc)?;

        let superclass = if self.match_token(&Token::LeftParen) {
            let super_loc = self.expect_identifier()?;
            let super_name = self.extract_identifier_name(&super_loc)?;
            self.expect(&Token::RightParen)?;
            Some(super_name)
        } else {
            None
        };

        if self.check(&Token::Colon) {
            self.advance();
        }

        self.skip_newlines();
        self.expect(&Token::Indent)?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut constructor = None;

        while !self.check(&Token::Dedent) && !self.is_at_end() {
            self.skip_newlines();

            if self.check(&Token::Class) {
                break;
            }

            if self.check(&Token::Dedent) {
                let mut temp_pos = self.position + 1;
                while temp_pos < self.tokens.len()
                    && matches!(self.tokens[temp_pos].token, Token::Newline)
                {
                    temp_pos += 1;
                }
                if temp_pos < self.tokens.len()
                    && matches!(self.tokens[temp_pos].token, Token::Indent)
                {
                    self.advance();
                    self.skip_newlines();
                    self.advance();
                    continue;
                }
                break;
            }

            let is_public = if self.match_token(&Token::Public) {
                true
            } else if self.match_token(&Token::Private) {
                false
            } else {
                true
            };
            let is_static = self.match_token(&Token::Static);

            if self.check(&Token::Func) {
                self.advance();
                let method_name_loc = self.expect_identifier()?;
                let method_name = self.extract_identifier_name(&method_name_loc)?;

                self.expect(&Token::LeftParen)?;
                let parameters = self.parse_parameters()?;

                self.skip_newlines();
                self.expect(&Token::RightParen)?;

                self.skip_layout_tokens();

                let return_type = if self.match_token(&Token::Arrow) {
                    self.skip_newlines();
                    Some(self.parse_type()?)
                } else {
                    None
                };

                if self.check(&Token::Colon) {
                    self.advance();
                }
                let body = self.parse_block()?;

                if method_name == "init" || method_name == "__init__" {
                    constructor = Some(Box::new(ASTNode::FunctionDeclaration {
                        name: method_name.clone(),
                        parameters,
                        return_type: None,
                        body: Box::new(body),
                    }));
                } else {
                    methods.push(ClassMethod {
                        name: method_name,
                        parameters,
                        return_type,
                        body: Box::new(body),
                        is_public,
                        is_static,
                    });
                }
            } else if self.check(&Token::Let)
                || self.check(&Token::Mut)
                || matches!(self.current()?.token, Token::Identifier(_))
            {
                let is_mutable = self.match_token(&Token::Mut);
                if !is_mutable {
                    self.match_token(&Token::Let);
                }

                let field_name_loc = self.expect_identifier()?;
                let field_name = self.extract_identifier_name(&field_name_loc)?;

                let field_type = if self.match_token(&Token::Colon) {
                    self.parse_type()?
                } else {
                    Type::Any
                };

                let default_value = if self.match_token(&Token::Equal) {
                    Some(Box::new(self.parse_expression()?))
                } else {
                    None
                };
                self.skip_newlines();
                fields.push(ClassField {
                    name: field_name,
                    field_type,
                    default_value,
                    is_public,
                });
            } else {
                return Err(format!(
                    "Expected field or method declaration in class at {}",
                    loc
                ));
            }
        }

        if !self.check(&Token::Class) {
            self.expect(&Token::Dedent)?;
        }

        Ok(ASTNode::ClassDeclaration {
            name,
            superclass,
            fields,
            methods,
            constructor,
            loc,
        })
    }

    fn parse_new_instance(&mut self) -> Result<ASTNode, String> {
        let loc = self.get_loc(self.current()?);
        self.expect(&Token::New)?;

        let name_loc = self.expect_identifier()?;
        let mut class_name = self.extract_identifier_name(&name_loc)?;

        while self.match_token(&Token::Dot) {
            let part_loc = self.expect_identifier()?;
            let part_name = self.extract_identifier_name(&part_loc)?;
            class_name.push('.');
            class_name.push_str(&part_name);
        }

        self.expect(&Token::LeftParen)?;
        let arguments = self.parse_arguments()?;

        Ok(ASTNode::NewInstance {
            class_name,
            arguments,
            loc,
        })
    }

    fn parse_circuit_declaration(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Circuit)?;

        let name_loc = self.expect_identifier()?;
        let name = self.extract_identifier_name(&name_loc)?;

        self.expect(&Token::LeftParen)?;
        let parameters = self.parse_parameters()?;

        self.skip_newlines();
        self.expect(&Token::RightParen)?;

        self.skip_layout_tokens();

        let return_type = if self.match_token(&Token::Arrow) {
            self.skip_newlines();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&Token::Colon)?;
        self.skip_newlines();

        let body = self.parse_block()?;

        Ok(ASTNode::CircuitDeclaration {
            name,
            parameters,
            return_type,
            body: Box::new(body),
        })
    }

    fn parse_parameters(&mut self) -> Result<Vec<Parameter>, String> {
        let mut params = Vec::new();

        self.skip_newlines();

        if self.check(&Token::RightParen) {
            return Ok(params);
        }

        loop {
            self.skip_newlines();
            let name_loc = self.expect_identifier()?;
            let name = self.extract_identifier_name(&name_loc)?;

            self.expect(&Token::Colon)?;
            let param_type = self.parse_type()?;

            params.push(Parameter { name, param_type });
            self.skip_newlines();

            if !self.match_token(&Token::Comma) {
                break;
            }
        }
        self.skip_newlines();
        Ok(params)
    }

    fn parse_arguments(&mut self) -> Result<Vec<ASTNode>, String> {
        let mut arguments = Vec::new();
        self.skip_newlines();
        if !self.check(&Token::RightParen) {
            loop {
                self.skip_newlines();
                arguments.push(self.parse_expression()?);
                self.skip_newlines();
                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.skip_newlines();
        self.expect(&Token::RightParen)?;
        Ok(arguments)
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let type_token = self.current()?.token.clone();

        let mut base_type = match type_token {
            Token::Int => {
                self.advance();
                Type::Int
            }
            Token::Int8 => {
                self.advance();
                Type::Int8
            }
            Token::Int16 => {
                self.advance();
                Type::Int16
            }
            Token::Int32 => {
                self.advance();
                Type::Int32
            }
            Token::Int64 => {
                self.advance();
                Type::Int64
            }
            Token::Uint => {
                self.advance();
                Type::Uint
            }
            Token::Float => {
                self.advance();
                Type::Float
            }
            Token::Float32 => {
                self.advance();
                Type::Float32
            }
            Token::Float64 => {
                self.advance();
                Type::Float64
            }
            Token::Bool => {
                self.advance();
                Type::Bool
            }
            Token::Bit => {
                self.advance();
                Type::Bit
            }
            Token::String => {
                self.advance();
                Type::String
            }
            Token::None => {
                self.advance();
                Type::None
            }
            Token::Any => {
                self.advance();
                Type::Any
            }
            Token::Quantum => {
                self.advance();
                Type::QuantumRegister(None)
            }
            Token::Identifier(name) => {
                self.advance();

                // Handle module-qualified types like ai.Dataset or math.Vector
                let mut full_name = name.clone();
                while self.check(&Token::Dot) {
                    self.advance(); // consume the dot
                    let next_name_loc = self.expect_identifier()?;
                    let next_name = self.extract_identifier_name(&next_name_loc)?;
                    full_name.push('.');
                    full_name.push_str(&next_name);
                }

                match full_name.as_str() {
                    "Qubit" => Type::Qubit,
                    "QuantumRegister" => Type::QuantumRegister(None),
                    _ => Type::Custom(full_name),
                }
            }
            _ => return Err(format!("Expected a type, but found {:?}", type_token)),
        };

        // Handle array types
        while self.match_token(&Token::LeftBracket) {
            if self.check(&Token::RightBracket) {
                self.advance();
                base_type = Type::Array(Box::new(base_type));
            } else {
                let size = self.parse_expression()?;
                self.expect(&Token::RightBracket)?;
                if let ASTNode::IntLiteral(n) = size {
                    base_type = Type::QuantumArray(Box::new(base_type), Some(n as usize));
                } else {
                    base_type = Type::QuantumArray(Box::new(base_type), None);
                }
            }
        }

        Ok(base_type)
    }

    fn parse_return(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Return)?;

        if self.check(&Token::Newline) || self.is_at_end() {
            self.skip_newlines();
            return Ok(ASTNode::Return(None));
        }

        let value = self.parse_expression()?;
        self.skip_newlines();

        Ok(ASTNode::Return(Some(Box::new(value))))
    }

    fn parse_if(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::If)?;

        let condition = self.parse_expression()?;
        self.expect(&Token::Colon)?;

        let then_block = if self.check(&Token::Newline) {
            self.skip_newlines();
            self.parse_block()?
        } else {
            let expr = self.parse_expression()?;
            self.skip_newlines();
            expr
        };

        let mut elif_blocks = Vec::new();
        while self.match_token(&Token::Elif) {
            let elif_condition = self.parse_expression()?;
            self.expect(&Token::Colon)?;

            let elif_body = if self.check(&Token::Newline) {
                self.skip_newlines();
                self.parse_block()?
            } else {
                let expr = self.parse_expression()?;
                self.skip_newlines();
                expr
            };

            elif_blocks.push((elif_condition, elif_body));
        }

        let else_block = if self.match_token(&Token::Else) {
            self.expect(&Token::Colon)?;

            let else_body = if self.check(&Token::Newline) {
                self.skip_newlines();
                self.parse_block()?
            } else {
                let expr = self.parse_expression()?;
                self.skip_newlines();
                expr
            };

            Some(Box::new(else_body))
        } else {
            None
        };

        Ok(ASTNode::If {
            condition: Box::new(condition),
            then_block: Box::new(then_block),
            elif_blocks,
            else_block,
        })
    }

    fn parse_match(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::Match)?;

        let value = self.parse_expression()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();

        self.expect(&Token::Indent)?;

        let mut cases = Vec::new();

        while !self.check(&Token::Dedent) && !self.is_at_end() {
            self.skip_newlines();

            if self.check(&Token::Dedent) {
                break;
            }

            self.expect(&Token::Case)?;

            let pattern = self.parse_pattern()?;
            self.expect(&Token::Arrow)?;

            let body = self.parse_expression()?;
            self.skip_newlines();

            cases.push((pattern, body));
        }

        self.expect(&Token::Dedent)?;

        Ok(ASTNode::Match {
            value: Box::new(value),
            cases,
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        if self.match_token(&Token::Identifier("_".to_string())) {
            return Ok(Pattern::Wildcard);
        }

        if let Token::Identifier(name) = &self.current()?.token {
            let id = name.clone();
            self.advance();
            return Ok(Pattern::Identifier(id));
        }

        let expr = self.parse_primary()?;
        Ok(Pattern::Literal(expr))
    }

    fn match_tokens_loc(&mut self, tokens: &[Token]) -> Option<TokenWithLocation> {
        for token in tokens {
            if self.check(token) {
                let matched_loc = self.current().unwrap().clone();
                self.advance();
                return Some(matched_loc);
            }
        }
        None
    }

    fn parse_for(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::For)?;

        let variable_loc = self.expect_identifier()?;
        let variable = self.extract_identifier_name(&variable_loc)?;
        self.expect(&Token::In)?;

        let iterator = self.parse_expression()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();

        let body = self.parse_block()?;

        Ok(ASTNode::For {
            variable,
            iterator: Box::new(iterator),
            body: Box::new(body),
        })
    }

    fn parse_while(&mut self) -> Result<ASTNode, String> {
        self.expect(&Token::While)?;

        let condition = self.parse_expression()?;
        self.expect(&Token::Colon)?;
        self.skip_newlines();

        let body = self.parse_block()?;

        Ok(ASTNode::While {
            condition: Box::new(condition),
            body: Box::new(body),
        })
    }
    fn debug_tokens(&self, start: usize, count: usize) {
        println!("=== DEBUG: Tokens from position {} ===", start);
        for i in start..std::cmp::min(start + count, self.tokens.len()) {
            println!("  [{}] {:?}", i, self.tokens[i]);
        }
        println!("=== END DEBUG ===");
    }

    fn parse_block(&mut self) -> Result<ASTNode, String> {
        self.skip_newlines();

        /* self.debug_current_context("parse_block entry"); */

        let mut statements = Vec::new();

        if !self.check(&Token::Indent) {
            return Err("Expected indented block after ':'".to_string());
        }
        self.advance();

        loop {
            self.skip_newlines();

            if self.check(&Token::Dedent) {
                let mut temp_pos = self.position + 1;

                while temp_pos < self.tokens.len()
                    && matches!(self.tokens[temp_pos].token, Token::Newline)
                {
                    temp_pos += 1;
                }

                if temp_pos < self.tokens.len()
                    && matches!(self.tokens[temp_pos].token, Token::Indent)
                {
                    self.advance();
                    self.skip_newlines();
                    self.advance();
                    continue;
                } else {
                    break;
                }
            }

            if self.is_at_end() {
                break;
            }

            statements.push(self.parse_statement()?);
        }

        if self.check(&Token::Dedent) {
            self.expect(&Token::Dedent)?;
        }

        Ok(ASTNode::Block(statements))
    }

    fn parse_expression(&mut self) -> Result<ASTNode, String> {
        self.parse_pipeline()
    }

    fn parse_pipeline(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_logical_or()?;
        while let Some(op_token) = self.match_tokens_loc(&[Token::PipeRight]) {
            let operator = BinaryOperator::Pipeline;
            let loc = self.get_loc(&op_token);
            let right = self.parse_logical_or()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_logical_or(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_logical_and()?;
        while let Some(op_token) = self.match_tokens_loc(&[Token::Or]) {
            let operator = BinaryOperator::Or;
            let loc = self.get_loc(&op_token);
            let right = self.parse_logical_and()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_equality()?;
        while let Some(op_token) = self.match_tokens_loc(&[Token::And]) {
            let operator = BinaryOperator::And;
            let loc = self.get_loc(&op_token);
            let right = self.parse_equality()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_comparison()?;
        while let Some(op_token) = self.match_tokens_loc(&[Token::EqualEqual, Token::NotEqual]) {
            let operator = match op_token.token {
                Token::EqualEqual => BinaryOperator::Equal,
                Token::NotEqual => BinaryOperator::NotEqual,
                _ => unreachable!(),
            };
            let loc = self.get_loc(&op_token);
            let right = self.parse_comparison()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_range()?;
        while let Some(op_token) = self.match_tokens_loc(&[
            Token::Less,
            Token::Greater,
            Token::LessEqual,
            Token::GreaterEqual,
        ]) {
            let operator = match op_token.token {
                Token::Less => BinaryOperator::Less,
                Token::Greater => BinaryOperator::Greater,
                Token::LessEqual => BinaryOperator::LessEqual,
                Token::GreaterEqual => BinaryOperator::GreaterEqual,
                _ => unreachable!(),
            };
            let loc = self.get_loc(&op_token);
            let right = self.parse_range()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_range(&mut self) -> Result<ASTNode, String> {
        let expr = self.parse_term()?;
        if let Some(op) = self.match_tokens(&[Token::Range, Token::RangeInclusive]) {
            let inclusive = matches!(op, Token::RangeInclusive);
            let end = self.parse_term()?;
            return Ok(ASTNode::Range {
                start: Box::new(expr),
                end: Box::new(end),
                inclusive,
            });
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_factor()?;
        while let Some(op_token) = self.match_tokens_loc(&[Token::Plus, Token::Minus]) {
            let operator = match op_token.token {
                Token::Plus => BinaryOperator::Add,
                Token::Minus => BinaryOperator::Sub,
                _ => unreachable!(),
            };
            let loc = self.get_loc(&op_token);
            let right = self.parse_factor()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_power()?;
        while let Some(op_token) =
            self.match_tokens_loc(&[Token::Star, Token::Slash, Token::Percent])
        {
            let operator = match op_token.token {
                Token::Star => BinaryOperator::Mul,
                Token::Slash => BinaryOperator::Div,
                Token::Percent => BinaryOperator::Mod,
                _ => unreachable!(),
            };
            let loc = self.get_loc(&op_token);
            let right = self.parse_power()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_tensor_product(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_unary()?;
        while let Some(op_token) = self.match_tokens_loc(&[Token::TensorProduct]) {
            let operator = BinaryOperator::TensorProduct;
            let loc = self.get_loc(&op_token);
            let right = self.parse_unary()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<ASTNode, String> {
        if let Some(op) = self.match_tokens(&[Token::Not, Token::Bang, Token::Minus, Token::Plus]) {
            let operator = match op {
                Token::Not => UnaryOperator::Not,
                Token::Bang => UnaryOperator::Not,
                Token::Minus => UnaryOperator::Minus,
                Token::Plus => UnaryOperator::Plus,
                _ => unreachable!(),
            };

            let operand = self.parse_unary()?;
            return Ok(ASTNode::Unary {
                operator,
                operand: Box::new(operand),
            });
        }

        self.parse_postfix()
    }

    fn parse_power(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_tensor_product()?;

        if let Some(op_token) = self.match_tokens_loc(&[Token::Caret]) {
            let operator = BinaryOperator::Power;
            let loc = self.get_loc(&op_token);
            let right = self.parse_power()?;
            expr = ASTNode::Binary {
                operator,
                left: Box::new(expr),
                right: Box::new(right),
                loc,
            };
        }

        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<ASTNode, String> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.check(&Token::LeftParen) {
                let loc = self.get_loc(self.current()?);
                self.advance();
                let arguments = self.parse_arguments()?;

                expr = ASTNode::FunctionCall {
                    callee: Box::new(expr),
                    arguments,
                    loc,
                    is_dagger: false,
                };
            } else if self.check(&Token::LeftBracket) {
                let loc = self.get_loc(self.current()?);
                self.advance();
                let index = self.parse_expression()?;
                self.expect(&Token::RightBracket)?;

                expr = ASTNode::ArrayAccess {
                    array: Box::new(expr),
                    index: Box::new(index),
                    loc,
                };
            } else if self.match_token(&Token::Dot) {
                let name_loc = self.expect_identifier()?;
                let member = self.extract_identifier_name(&name_loc)?;

                if self.check(&Token::LeftParen) {
                    let loc = self.get_loc(self.current()?);
                    self.advance();
                    let arguments = self.parse_arguments()?;

                    expr = ASTNode::MethodCall {
                        object: Box::new(expr),
                        method_name: member,
                        arguments,
                        loc,
                    };
                } else {
                    expr = ASTNode::MemberAccess {
                        object: Box::new(expr),
                        member,
                    };
                }
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn get_loc(&self, token_loc: &TokenWithLocation) -> Loc {
        Loc {
            line: token_loc.line,
            column: token_loc.column,
        }
    }

    fn parse_primary(&mut self) -> Result<ASTNode, String> {
        while self.check(&Token::Newline) {
            self.advance();
        }

        if self.check(&Token::Indent) || self.check(&Token::Dedent) {
            let current_loc = self.current()?.clone();
            return Err(format!(
                "Unexpected indentation token at {}: {:?}. This usually means a statement is incomplete or malformed.",
                self.get_loc(&current_loc),
                current_loc.token
            ));
        }
        let current_loc = self.current()?.clone();
        let token = &current_loc.token;
        let loc = self.get_loc(&current_loc);

        match token {
            Token::If => self.parse_if(),
            Token::Print => {
                self.advance();
                Ok(ASTNode::Identifier {
                    name: "print".to_string(),
                    loc,
                })
            }
            Token::Echo => {
                self.advance();
                Ok(ASTNode::Identifier {
                    name: "echo".to_string(),
                    loc,
                })
            }


            Token::Identifier(name) => {
                self.advance();

                Ok(ASTNode::Identifier {
                    name: name.clone(),
                    loc,
                })
            }
            Token::New => self.parse_new_instance(),

            Token::Dagger => {
                self.advance();
                self.expect(&Token::LeftParen)?;
                let callee_expr = self.parse_expression()?;
                self.expect(&Token::RightParen)?;

                let call_loc = self.get_loc(self.current()?);
                if !self.match_token(&Token::LeftParen) {
                    return Err(format!(
                        "Error at {}: Expected '(' to call the daggered circuit.",
                        call_loc
                    ));
                }

                let arguments = self.parse_arguments()?;
                Ok(ASTNode::FunctionCall {
                    callee: Box::new(callee_expr),
                    arguments,
                    loc: call_loc,
                    is_dagger: true,
                })
            }

            Token::Measure => {
                self.advance();
                self.expect(&Token::LeftParen)?;
                let qubit = self.parse_expression()?;
                self.expect(&Token::RightParen)?;
                Ok(ASTNode::Measure(Box::new(qubit)))
            }

            Token::IntLiteral(n) => {
                self.advance();
                Ok(ASTNode::IntLiteral(*n))
            }
            Token::FloatLiteral(f) => {
                self.advance();
                Ok(ASTNode::FloatLiteral(*f))
            }
            Token::StringLiteral(s) => {
                self.advance();
                Ok(ASTNode::StringLiteral(s.clone()))
            }
            Token::True => {
                self.advance();
                Ok(ASTNode::BoolLiteral(true))
            }
            Token::False => {
                self.advance();
                Ok(ASTNode::BoolLiteral(false))
            }
            Token::None => {
                self.advance();
                Ok(ASTNode::NoneLiteral)
            }
            Token::KetState(state) => {
                self.advance();
                Ok(ASTNode::QuantumKet(state.clone()))
            }
            Token::BraState(state) => {
                self.advance();
                Ok(ASTNode::QuantumBra(state.clone()))
            }

            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(&Token::RightParen)?;
                Ok(expr)
            }
            Token::LeftBracket => {
                self.advance();
                self.parse_array_literal()
            }
            Token::LeftBrace => {
                self.advance();
                self.parse_dict_literal()
            }
            Token::New => self.parse_new_instance(),
            Token::Identifier(name) if name == "self" => {
                self.advance();
                Ok(ASTNode::SelfRef { loc })
            }

            Token::X
            | Token::Y
            | Token::Z
            | Token::S
            | Token::T
            | Token::Hadamard
            | Token::Cnot
            | Token::Swap
            | Token::Reset
            | Token::CZ
            | Token::CS
            | Token::CT
            | Token::CCX
            | Token::Toffoli
            | Token::RX
            | Token::RY
            | Token::RZ
            | Token::U
            | Token::CPhase => {
                let name = format!("{:?}", token).to_lowercase();
                self.advance();
                Ok(ASTNode::Identifier { name, loc })
            }

            _ => Err(format!(
                "Unexpected token in expression at {}: {:?}",
                loc, token
            )),
        }
    }

    fn skip_layout_tokens(&mut self) {
        while self.check(&Token::Newline)
            || self.check(&Token::Indent)
            || self.check(&Token::Dedent)
        {
            self.advance();
        }
    }

    fn parse_dict_literal(&mut self) -> Result<ASTNode, String> {
        let mut pairs = Vec::new();

        self.skip_layout_tokens();

        if self.check(&Token::RightBrace) {
            self.advance();
            return Ok(ASTNode::DictLiteral(pairs));
        }

        loop {
            let key_expr = self.parse_expression()?;
            self.expect(&Token::Colon)?;
            let value_expr = self.parse_expression()?;
            pairs.push((key_expr, value_expr));

            self.skip_layout_tokens();

            if self.check(&Token::RightBrace) {
                break;
            }

            self.expect(&Token::Comma)?;
            self.skip_layout_tokens();

            if self.check(&Token::RightBrace) {
                break;
            }
        }

        self.expect(&Token::RightBrace)?;

        Ok(ASTNode::DictLiteral(pairs))
    }

    fn current(&self) -> Result<&TokenWithLocation, String> {
        if self.is_at_end() {
            Err("Unexpected end of file".to_string())
        } else {
            Ok(&self.tokens[self.position])
        }
    }

    fn advance(&mut self) {
        if !self.is_at_end() {
            self.position += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.tokens.len()
            || matches!(
                self.tokens.get(self.position).map(|t| &t.token),
                Some(Token::Eof)
            )
    }

    fn check(&self, token: &Token) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.tokens[self.position].token) == std::mem::discriminant(token)
    }

    fn match_token(&mut self, token: &Token) -> bool {
        if self.check(token) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_tokens(&mut self, tokens: &[Token]) -> Option<Token> {
        for token in tokens {
            if self.check(token) {
                let matched = self.tokens[self.position].token.clone();
                self.advance();
                return Some(matched);
            }
        }
        None
    }

    fn expect(&mut self, token: &Token) -> Result<(), String> {
        if self.check(token) {
            self.advance();
            Ok(())
        } else {
            let (loc, current_str) = if self.position >= self.tokens.len() {
                let last_loc = if !self.tokens.is_empty() {
                    let last = self.tokens.last().unwrap();

                    Loc {
                        line: last.line,
                        column: last.column + 1,
                    }
                } else {
                    Loc { line: 1, column: 1 }
                };
                (last_loc, "EOF".to_string())
            } else {
                let curr = &self.tokens[self.position];
                (
                    Loc {
                        line: curr.line,
                        column: curr.column,
                    },
                    format!("{:?}", curr.token),
                )
            };

            Err(format!(
                "Syntax Error at {}: Expected {:?}, but found {}",
                loc, token, current_str
            ))
        }
    }

    fn extract_identifier_name(&self, token_loc: &TokenWithLocation) -> Result<String, String> {
        match &token_loc.token {
            Token::Identifier(name) => Ok(name.clone()),
            Token::Hadamard => Ok("hadamard".to_string()),
            Token::Cnot => Ok("cnot".to_string()),
            Token::X => Ok("x".to_string()),
            Token::Y => Ok("y".to_string()),
            Token::Z => Ok("z".to_string()),
            Token::S => Ok("s".to_string()),
            Token::T => Ok("t".to_string()),
            Token::Swap => Ok("swap".to_string()),
            Token::Reset => Ok("reset".to_string()),
            Token::CZ => Ok("cz".to_string()),
            Token::CS => Ok("cs".to_string()),
            Token::CT => Ok("ct".to_string()),
            Token::CPhase => Ok("cphase".to_string()),
            Token::U => Ok("u".to_string()),
            Token::RX => Ok("rx".to_string()),
            Token::RY => Ok("ry".to_string()),
            Token::RZ => Ok("rz".to_string()),
            Token::CCX => Ok("ccx".to_string()),
            Token::Toffoli => Ok("toffoli".to_string()),
            _ => Err(format!(
                "Expected Identifier token, found {:?}",
                token_loc.token
            )),
        }
    }

    fn expect_identifier(&mut self) -> Result<TokenWithLocation, String> {
        let current_loc = self.current()?;

        match &current_loc.token {
            Token::Identifier(_)
            | Token::Hadamard
            | Token::Cnot
            | Token::X
            | Token::Y
            | Token::Z
            | Token::S
            | Token::T
            | Token::Swap
            | Token::Reset
            | Token::CZ
            | Token::CS
            | Token::CT
            | Token::CPhase
            | Token::U
            | Token::RX
            | Token::RY
            | Token::RZ
            | Token::CCX
            | Token::Toffoli => {
                let owned_token_loc = current_loc.clone();
                self.advance();
                Ok(owned_token_loc)
            }
            _ => Err(format!(
                "Syntax Error: Expected Identifier or Gate name, found {:?}",
                current_loc.token
            )),
        }
    }

    fn skip_newlines(&mut self) {
        while self.match_token(&Token::Newline) {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_source(source: &str) -> Result<ASTNode, String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_let_declaration() {
        let source = "let x = 42";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert_eq!(statements.len(), 1);
            if let ASTNode::LetDeclaration {
                name,
                value,
                is_mutable,
                ..
            } = &statements[0]
            {
                assert_eq!(name, "x");
                assert_eq!(*is_mutable, false);
                assert!(matches!(**value, ASTNode::IntLiteral(42)));
            } else {
                panic!("Expected LetDeclaration");
            }
        }
    }

    #[test]
    fn test_quantum_declaration() {
        let source = "quantum q = |0}";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert_eq!(statements.len(), 1);
            if let ASTNode::QuantumDeclaration {
                name,
                initial_state,
                ..
            } = &statements[0]
            {
                assert_eq!(name, "q");
                assert!(initial_state.is_some());
                if let Some(state) = initial_state {
                    assert!(matches!(**state, ASTNode::QuantumKet(ref s) if s == "0"));
                }
            } else {
                panic!("Expected QuantumDeclaration");
            }
        }
    }

    #[test]
    fn test_tensor_product() {
        let source = "let result = q1 *** q2";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::LetDeclaration { value, .. } = &statements[0] {
                if let ASTNode::Binary {
                    operator,
                    left,
                    right,
                    ..
                } = &**value
                {
                    assert_eq!(*operator, BinaryOperator::TensorProduct);
                    assert!(matches!(**left, ASTNode::Identifier{ref name, .. } if name == "q1"));
                    assert!(matches!(**right, ASTNode::Identifier{ref name, .. } if name == "q2"));
                } else {
                    panic!("Expected Binary TensorProduct");
                }
            }
        }
    }

    #[test]
    fn test_function_declaration() {
        let source = "func add(x: Int, y: Int) -> Int:\n    return x + y";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert_eq!(statements.len(), 1);
            if let ASTNode::FunctionDeclaration {
                name,
                parameters,
                return_type,
                ..
            } = &statements[0]
            {
                assert_eq!(name, "add");
                assert_eq!(parameters.len(), 2);
                assert!(matches!(return_type, Some(Type::Int)));
            } else {
                panic!("Expected FunctionDeclaration");
            }
        }
    }

    #[test]
    fn test_if_statement() {
        let source = "if x > 5:\n    print(\"yes\")";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert!(matches!(statements[0], ASTNode::If { .. }));
        } else {
            panic!("Expected Program node");
        }
    }

    #[test]
    fn test_if_elif_else() {
        let source = "if x > 5:\n    print(\"big\")\nelif x > 0:\n    print(\"small\")\nelse:\n    print(\"zero or negative\")";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::If {
                elif_blocks,
                else_block,
                ..
            } = &statements[0]
            {
                assert_eq!(elif_blocks.len(), 1);
                assert!(else_block.is_some());
            } else {
                panic!("Expected If statement");
            }
        }
    }

    #[test]
    fn test_dict_literal() {
        let source = "let d = { 1: \"a\", \"b\": 2 }";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::LetDeclaration { value, .. } = &statements[0] {
                if let ASTNode::DictLiteral(pairs) = &**value {
                    assert_eq!(pairs.len(), 2);
                    assert!(matches!(pairs[0].0, ASTNode::IntLiteral(1)));
                    assert!(matches!(&pairs[1].0, ASTNode::StringLiteral(ref s) if s == "b"));
                } else {
                    panic!("Expected DictLiteral");
                }
            }
        }
    }

    #[test]
    fn test_apply_statement() {
        let source = "apply Hadamard(q[0])";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert!(matches!(statements[0], ASTNode::Apply { .. }));
        }
    }

    #[test]
    fn test_controlled_gate() {
        let source = "apply controlled(X)(q[0], q[1])";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::Apply { gate_expr, .. } = &statements[0] {
                assert!(matches!(**gate_expr, ASTNode::Controlled { .. }));
            } else {
                panic!("Expected Apply with Controlled gate");
            }
        }
    }

    #[test]
    fn test_dagger_gate() {
        let source = "apply dagger(S)(q[0])";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::Apply { gate_expr, .. } = &statements[0] {
                assert!(matches!(**gate_expr, ASTNode::Dagger { .. }));
            } else {
                panic!("Expected Apply with Dagger gate");
            }
        }
    }

    #[test]
    fn test_parameterized_gate() {
        let source = "apply RX(3.14)(q[0])";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::Apply { gate_expr, .. } = &statements[0] {
                assert!(matches!(**gate_expr, ASTNode::ParameterizedGate { .. }));
            } else {
                panic!("Expected Apply with ParameterizedGate");
            }
        }
    }

    #[test]
    fn test_power_operator() {
        let source = "let result = 2 ^ 3";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::LetDeclaration { value, .. } = &statements[0] {
                if let ASTNode::Binary { operator, .. } = &**value {
                    assert_eq!(*operator, BinaryOperator::Power);
                } else {
                    panic!("Expected Binary Power operation");
                }
            }
        }
    }

    #[test]
    fn test_import_statement() {
        let source = "import \"math.qc\" as math";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert!(matches!(statements[0], ASTNode::Import { .. }));
        }
    }

    #[test]
    fn test_from_import() {
        let source = "from \"math.qc\" import PI, sin";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert!(matches!(statements[0], ASTNode::FromImport { .. }));
        }
    }

    #[test]
    fn test_try_catch() {
        let source = "Try:\n    let x = 10\nCatch err:\n    print(err)";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            assert!(matches!(statements[0], ASTNode::TryCatch { .. }));
        }
    }

    #[test]
    fn test_array_literal() {
        let source = "let arr = [1, 2, 3]";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::LetDeclaration { value, .. } = &statements[0] {
                if let ASTNode::ArrayLiteral(elements) = &**value {
                    assert_eq!(elements.len(), 3);
                } else {
                    panic!("Expected ArrayLiteral");
                }
            }
        }
    }

    #[test]
    fn test_range_expression() {
        let source = "let r = 0..10";
        let ast = parse_source(source).unwrap();

        if let ASTNode::Program(statements) = ast {
            if let ASTNode::LetDeclaration { value, .. } = &statements[0] {
                assert!(matches!(**value, ASTNode::Range { .. }));
            }
        }
    }
}
