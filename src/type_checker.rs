// src/type_checker.rs

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::string::String;
use crate::parser::ast::{ASTNode, Type, BinaryOperator, UnaryOperator, ImportSpec};


use crate::parser::ast::ImportPath;
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::fs;
use crate::parser::ast::Loc;


#[derive(Debug, Clone, PartialEq)]
pub struct TypeInfo {
    pub var_type: Type,
    pub is_mutable: bool,
}

#[derive(Debug, Clone)]
pub struct TypeEnvironment {
    store: HashMap<String, TypeInfo>,
    outer: Option<Rc<RefCell<TypeEnvironment>>>,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        TypeEnvironment { store: HashMap::new(), outer: Option::None }
    }

    pub fn new_enclosed(outer_env: Rc<RefCell<TypeEnvironment>>) -> Self {
        TypeEnvironment { store: HashMap::new(), outer: Some(outer_env) }
    }

    pub fn get(&self, name: &str) -> Option<TypeInfo> {
        if let Some(t) = self.store.get(name) {
            return Some(t.clone());
        }
        if let Some(outer) = &self.outer {
            return outer.borrow().get(name);
        }
        Option::None
    }

    pub fn set(&mut self, name: String, t: TypeInfo) {
        self.store.insert(name, t);
    }
}


pub struct TypeChecker;

impl TypeChecker {

    pub fn prefill_environment(env: &Rc<RefCell<TypeEnvironment>>) {
        let mut env_mut = env.borrow_mut();

        let immut = |t: Type| TypeInfo { var_type: t, is_mutable: false };

        let any = Type::Any;
        let none = Type::None;
        let qubit_type = Type::Qubit;
        let none_type = Box::new(none.clone());

        // --- Built-ins ---
        env_mut.set("print".to_string(), immut(Type::Function(vec![], Box::new(none.clone()))));
        env_mut.set("debug_state".to_string(), immut(Type::Function(vec![Type::QuantumRegister(None)], none_type.clone())));
        env_mut.set("assert".to_string(), immut(
            Type::Function(vec![Type::Bool, Type::String], none_type.clone())
        ));
        env_mut.set("maybe".to_string(), immut(
            Type::Function(vec![Type::Any, Type::Float], Box::new(Type::Any))
        ));
        env_mut.set("sample".to_string(), immut(
            Type::Function(vec![Type::Any], Box::new(Type::Any))
        ));
        env_mut.set("echo".to_string(), immut(
            Type::Function(vec![Type::Any], Box::new(Type::Any))
        ));
        env_mut.set("len".to_string(), immut(Type::Function(vec![any.clone()], Box::new(Type::Int))));
        env_mut.set("type_of".to_string(), immut(Type::Function(vec![any.clone()], Box::new(Type::String))));
        env_mut.set("to_string".to_string(), immut(Type::Function(vec![any.clone()], Box::new(Type::String))));
        env_mut.set("to_int".to_string(), immut(Type::Function(vec![any.clone()], Box::new(Type::Int))));
        env_mut.set("to_float".to_string(), immut(Type::Function(vec![any.clone()], Box::new(Type::Float))));

        // --- Single-Qubit Gates ---
        let single_qubit_gate = Type::Function(vec![qubit_type.clone()], none_type.clone());
        env_mut.set("hadamard".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("x".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("y".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("z".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("s".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("t".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("reset".to_string(), immut(single_qubit_gate.clone()));

        // --- Multi-Qubit Gates ---
        let two_qubit_gate = Type::Function(vec![qubit_type.clone(), qubit_type.clone()], none_type.clone());
        env_mut.set("cnot".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("swap".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("cz".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("cs".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("ct".to_string(), immut(two_qubit_gate.clone()));

        let three_qubit_gate = Type::Function(
            vec![qubit_type.clone(), qubit_type.clone(), qubit_type.clone()],
            none_type.clone()
        );
        env_mut.set("ccx".to_string(), immut(three_qubit_gate.clone()));
        env_mut.set("toffoli".to_string(), immut(three_qubit_gate));

        // --- Parameterized Gates ---
        let cphase_gate = Type::Function(
            vec![Type::Float, qubit_type.clone(), qubit_type.clone()],
            none_type.clone()
        );
        env_mut.set("cphase".to_string(), immut(cphase_gate));

        let u_gate = Type::Function(
            vec![Type::Float, Type::Float, Type::Float, qubit_type.clone()],
            none_type.clone()
        );
        env_mut.set("u".to_string(), immut(u_gate));

        let parameterized_gate = Type::Function(
            vec![Type::Float, qubit_type.clone()],
            none_type.clone()
        );
        env_mut.set("rx".to_string(), immut(parameterized_gate.clone()));
        env_mut.set("ry".to_string(), immut(parameterized_gate.clone()));
        env_mut.set("rz".to_string(), immut(parameterized_gate.clone()));
    }


    pub fn check_program(node: &ASTNode) -> Result<(), String> {
        if let ASTNode::Program(statements) = node {
            let env = Rc::new(RefCell::new(TypeEnvironment::new()));
            Self::prefill_environment(&env);
            for stmt in statements {
                Self::check(stmt, &env, Option::None)?;
            }
            Ok(())
        } else {
            Err("Expected Program node".to_string())
        }
    }

    fn immutable_info(t: Type) -> TypeInfo {
        TypeInfo { var_type: t, is_mutable: false }
    }

    fn check_module(path: &ImportPath) -> Result<HashMap<String, Type>, String> {
        let file_path = match path {

            ImportPath::File(f) => {
                if f.ends_with(".qc") || f.contains('/') || f.contains('\\') {

                    f.clone()
                } else {

                    format!("q_packages/{}/init.qc", f)
                }
            }
            ImportPath::Module(m) => {

                m.join("/") + ".qc"
            }
        };

        let source = fs::read_to_string(&file_path)
            .map_err(|e| format!("Type Check Error: Failed to read module '{}': {}", file_path, e))?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().map_err(|e| format!("Module Lexer Error: {}", e))?;
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().map_err(|e| format!("Module Parser Error: {}", e))?;

        let module_env = Rc::new(RefCell::new(TypeEnvironment::new()));
        Self::prefill_environment(&module_env);

        if let ASTNode::Program(statements) = ast {
            for stmt in statements {
                Self::check(&stmt, &module_env, None)?;
            }
        } else {
            return Err("Module root is not a Program node".to_string());
        }

        let module_types = module_env.borrow().store.iter()
            .map(|(k, v)| (k.clone(), v.var_type.clone()))
            .collect();

        Ok(module_types)
    }


    fn check_gate_expression(
        node: &ASTNode,
        env: &Rc<RefCell<TypeEnvironment>>
    ) -> Result<Type, String> {
        let loc = match node {
            ASTNode::Gate { loc, .. } |ASTNode::ParameterizedGate { loc, .. } | ASTNode::Controlled { loc, .. } | ASTNode::Dagger { loc, .. } => *loc,
            _ => Loc { line: 0, column: 0 },
        };

        match node {
            ASTNode::Gate { name, loc } => {
                let gate_name_lower = name.to_lowercase();

                if let Some(info) = env.borrow().get(&gate_name_lower) {
                    if let Type::Function(..) = &info.var_type {
                        return Ok(info.var_type.clone());
                    }
                }


                Err(format!("Type Error at {}: Unknown quantum gate '{}'.", loc, name))
            }

            ASTNode::Dagger { gate_expr, .. } => {

                let inner_gate_type = Self::check_gate_expression(gate_expr, env)?;

                if let Type::Function(..) = inner_gate_type {
                    Ok(inner_gate_type)
                } else {
                    Err(format!("Type Error at {}: 'dagger' can only be applied to a gate or circuit.", loc))
                }
            }

            ASTNode::Controlled { gate_expr, loc } => {

                let inner_gate_type = Self::check_gate_expression(gate_expr, env)?;

                match inner_gate_type {
                    Type::Function(mut params, ret_type) => {

                        if params.is_empty() {
                            return Err(format!("Type Error at {}: 'controlled' target gate must take at least one argument.", loc));
                        }


                        params.insert(0, Type::Qubit);


                        Ok(Type::Function(params, ret_type))
                    }
                    _ => Err(format!("Type Error at {}: 'controlled' can only be applied to a callable gate.", loc))
                }
            }

            ASTNode::ParameterizedGate { name, parameters, loc } => {
            let gate_name_lower = name.to_lowercase();


            let gate_info = env.borrow().get(&gate_name_lower)
                .ok_or_else(|| format!("Type Error at {}: Unknown parameterized gate '{}'.", loc, name))?;

            match gate_info.var_type {
                Type::Function(param_types, return_type) => {


                    if parameters.len() > param_types.len() {
                        return Err(format!(
                            "Type Error at {}: Gate '{}' takes at most {} parameters, but got {}",
                            loc, name, param_types.len(), parameters.len()
                        ));
                    }


                    for (i, param_expr) in parameters.iter().enumerate() {
                        let param_type = Self::check(param_expr, env, None)?;
                        let expected_type = &param_types[i];


                        if param_type != *expected_type &&
                           !(*expected_type == Type::Float && param_type == Type::Int) {
                            return Err(format!(
                                "Type Error at {}: Parameter {} of gate '{}' has wrong type. Expected {:?}, got {:?}",
                                loc, i + 1, name, expected_type, param_type
                            ));
                        }
                    }


                    let remaining_params = param_types[parameters.len()..].to_vec();
                    Ok(Type::Function(remaining_params, return_type))
                }
                _ => Err(format!("Type Error at {}: '{}' is not a parameterized gate.", loc, name))
            }
        }

            _ => Err(format!("Type Error: This expression is not a valid gate."))
        }
    }



    pub fn check(node: &ASTNode, env: &Rc<RefCell<TypeEnvironment>>, expected_return_type: Option<&Type>) -> Result<Type, String> {

        match node {
            // --- Literals ---
            ASTNode::IntLiteral(_) => Ok(Type::Int),
            ASTNode::FloatLiteral(_) => Ok(Type::Float),
            ASTNode::StringLiteral(_) => Ok(Type::String),
            ASTNode::BoolLiteral(_) => Ok(Type::Bool),
            ASTNode::NoneLiteral => Ok(Type::None),
            ASTNode::DictLiteral(_) => Ok(Type::Dict),
            ASTNode::QuantumKet(_) => Ok(Type::QuantumRegister(Some(1))),
            ASTNode::QuantumBra(_) => Err("Bra notation is not yet supported.".to_string()),

            // --- Declarations ---
            ASTNode::LetDeclaration { name, type_annotation, value, is_mutable, ..} => {
                let value_type = Self::check(value, env, Option::None)?;

                let final_type = match type_annotation {
                    Some(expected_type) => {
                        if value_type != *expected_type &&
                           value_type != Type::None &&
                           *expected_type != Type::Any {
                            if let (Type::QuantumRegister(_), Type::QuantumRegister(None)) = (&value_type, expected_type) {
                                // OK
                            } else {
                                return Err(format!(
                                    "Type Error: Variable '{}' is annotated as {:?} but is being assigned a value of type {:?}",
                                    name, expected_type, value_type
                                ));
                            }
                        }
                        expected_type.clone()
                    }
                    Option::None => value_type,
                };

                let info = TypeInfo { var_type: final_type, is_mutable: *is_mutable };
                env.borrow_mut().set(name.clone(), info);
                Ok(Type::None)
            }

            ASTNode::Assignment { target, value } => {
                let new_type = Self::check(value, env, Option::None)?;

                match target.as_ref() {

                    ASTNode::Identifier { name, .. } => {
                        let original_info = match env.borrow().get(name) {
                            Some(info) => info,
                            Option::None => return Err(format!("Type Error: Cannot assign to undefined variable '{}'", name)),
                        };

                        if !original_info.is_mutable {
                            return Err(format!("Mutability Error: Cannot assign to immutable variable '{}'.", name));
                        }

                        if original_info.var_type != new_type &&
                        original_info.var_type != Type::Any &&
                        new_type != Type::None {
                            if let (Type::QuantumRegister(_), Type::QuantumRegister(None)) = (&new_type, &original_info.var_type) {
                                // OK
                            } else {
                                return Err(format!(
                                    "Type Error: Mismatched types in assignment. Cannot assign type {:?} to variable '{}' of type {:?}",
                                    new_type, name, original_info.var_type
                                ));
                            }
                        }
                        Ok(Type::None)
                    }


                    ASTNode::ArrayAccess { array, index, loc } => {
                        let array_type = Self::check(array, env, Option::None)?;
                        let index_type = Self::check(index, env, Option::None)?;

                        match array_type {
                            Type::Dict => {

                                if index_type != Type::String &&
                                index_type != Type::Int &&
                                index_type != Type::Bool {
                                    return Err(format!(
                                        "Type Error at {}: Dictionary keys must be String, Int, or Bool, got {:?}",
                                        loc, index_type
                                    ));
                                }

                                Ok(Type::None)
                            }
                            Type::Array(inner_type) => {

                                if index_type != Type::Int {
                                    return Err(format!(
                                        "Type Error at {}: Array index must be Int, got {:?}",
                                        loc, index_type
                                    ));
                                }

                                if new_type != *inner_type && *inner_type != Type::Any {
                                    return Err(format!(
                                        "Type Error at {}: Cannot assign {:?} to array of {:?}",
                                        loc, new_type, inner_type
                                    ));
                                }
                                Ok(Type::None)
                            }
                            _ => Err(format!(
                                "Type Error at {}: Cannot perform subscript assignment on type {:?}",
                                loc, array_type
                            ))
                        }
                    }

                    _ => Err("Type Error: Assignment target must be an identifier or subscript expression.".to_string())
                }
            }
            ASTNode::Identifier { name, loc } => {
                match env.borrow().get(name) {
                    Some(info) => Ok(info.var_type.clone()),
                    Option::None => Err(format!("Type Error at {}: Undefined variable '{}'", loc, name)),
                }
            }


            ASTNode::FunctionCall { callee, arguments, loc, .. } => {
                let callee_type = Self::check(callee, env, Option::None)?;

                let name = format!("{:?}", callee);
                match callee_type {
                    Type::Function(param_types, return_type) => {
                        if arguments.len() > 0 && param_types.len() == 0 {

                        } else if arguments.len() != param_types.len() {
                            return Err(format!(
                                "Type Error at {}: Function '{}' expected {} arguments, but got {}",
                                loc, name, param_types.len(), arguments.len()
                            ));
                        }
                        for (i, arg_node) in arguments.iter().enumerate() {
                            if i >= param_types.len() { break; }
                            let arg_type = Self::check(arg_node, env, Option::None)?;
                            let expected_type = &param_types[i];

                            if arg_type != *expected_type && *expected_type != Type::Any {
                                if let (Type::QuantumRegister(_), Type::QuantumRegister(None)) = (&arg_type, expected_type) {
                                    // OK
                                } else {
                                    return Err(format!(
                                        "Type Error at {}: Argument {} of function call is wrong type. Expected {:?}, got {:?}",
                                        loc, (i + 1), expected_type, arg_type
                                    ));
                                }
                            }
                        }
                        Ok(*return_type)
                    }
                    _ => Err(format!(
                        "Type Error at {}: Cannot call a value of type {:?} as a function.",
                        loc, callee_type
                    )),
                }
            }




            ASTNode::Binary { operator, left, right, loc } => {
                let left_type = Self::check(left, env, Option::None)?;
                let right_type = Self::check(right, env, Option::None)?;

                if *operator == BinaryOperator::TensorProduct {
                    match (left_type.clone(), right_type.clone()) {
                        (Type::QuantumRegister(Some(s1)), Type::QuantumRegister(Some(s2))) => {
                            return Ok(Type::QuantumRegister(Some(s1 + s2)));
                        }
                        (Type::QuantumRegister(_), Type::QuantumRegister(_)) => {
                            return Ok(Type::QuantumRegister(None));
                        }
                        _ => return Err(format!(
                            "Type Error at {}: Tensor product '***' is only defined for quantum registers, got {:?} and {:?}",
                            loc, left_type, right_type
                        )),
                    }
                }

                match operator {
                    BinaryOperator::Add => {
                        match (&left_type, &right_type) {
                            (Type::Int, Type::Int) => Ok(Type::Int),
                            (Type::Float, Type::Float) => Ok(Type::Float),
                            (Type::Int, Type::Float) => Ok(Type::Float),
                            (Type::Float, Type::Int) => Ok(Type::Float),
                            (Type::String, Type::String) => Ok(Type::String),
                            _ => Err(format!("Type Error at {}: Cannot add types {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::Sub | BinaryOperator::Mul | BinaryOperator::Div | BinaryOperator::Mod => {
                         match (&left_type, &right_type) {
                            (Type::Int, Type::Int) => Ok(Type::Int),
                            (Type::Float, Type::Float) => Ok(Type::Float),
                            (Type::Int, Type::Float) => Ok(Type::Float),
                            (Type::Float, Type::Int) => Ok(Type::Float),
                            _ => Err(format!("Type Error at {}: Cannot perform arithmetic on types {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::Power => {
                        match (&left_type, &right_type) {
                            (Type::Int, Type::Int) => Ok(Type::Int),
                            (Type::Float, Type::Float) => Ok(Type::Float),
                            (Type::Int, Type::Float) => Ok(Type::Float),
                            (Type::Float, Type::Int) => Ok(Type::Float),
                            _ => Err(format!("Type Error at {}: Power operator (^) requires numeric types, got {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual => Ok(Type::Bool),
                    BinaryOperator::Less | BinaryOperator::Greater | BinaryOperator::LessEqual | BinaryOperator::GreaterEqual => {
                        match (&left_type, &right_type) {
                            (Type::Int, Type::Int) | (Type::Float, Type::Float) => Ok(Type::Bool),
                            _ => Err(format!("Type Error at {}: Cannot perform ordered comparison on types {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::And | BinaryOperator::Or => {
                        match (&left_type, &right_type) {
                            (Type::Bool, Type::Bool) => Ok(Type::Bool),
                            _ => Err(format!("Type Error at {}: Logical operators 'and'/'or' require two booleans, got {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    _ => Err(format!("Type checking not implemented for operator {:?}", operator))
                }
            }

            ASTNode::Unary { operator, operand } => {
                let operand_type = Self::check(operand, env, Option::None)?;
                match operator {
                    UnaryOperator::Not => {
                        if operand_type != Type::Bool {
                            return Err(format!("Type Error: Unary operator '!' cannot be applied to type {:?}", operand_type));
                        }
                        Ok(Type::Bool)
                    }
                    UnaryOperator::Minus | UnaryOperator::Plus => {
                        if operand_type != Type::Int && operand_type != Type::Float {
                             return Err(format!("Type Error: Unary operator '-' or '+' cannot be applied to type {:?}", operand_type));
                        }
                        Ok(operand_type)
                    }
                }
            }

            ASTNode::QuantumDeclaration { name, size, initial_state } => {
                let register_type: Type;
                if let Some(size_expr) = size {
                    let size_type = Self::check(size_expr, env, Option::None)?;
                    if size_type != Type::Int {
                        return Err(format!("Type Error: Quantum register size must be an Int, but got {:?}", size_type));
                    }
                    let size_val = if let ASTNode::IntLiteral(n) = &**size_expr { Some(*n as usize) } else { None };
                    register_type = Type::QuantumRegister(size_val);
                } else if let Some(state_expr) = initial_state {
                    let init_type = Self::check(state_expr, env, Option::None)?;
                    if let Type::QuantumRegister(size_opt) = init_type {
                        register_type = Type::QuantumRegister(size_opt);
                    } else {
                        return Err(format!("Type Error: Initial state for a quantum register must be another quantum register, got {:?}", init_type));
                    }
                } else {
                    register_type = Type::QuantumRegister(Some(1));
                }
                env.borrow_mut().set(name.clone(), Self::immutable_info(register_type));
                Ok(Type::None)
            }

            ASTNode::ArrayAccess { array, index, loc } => {
                let array_type = Self::check(array, env, Option::None)?;
                let index_type = Self::check(index, env, Option::None)?;
                if index_type != Type::Int {
                    return Err(format!("Type Error at {}: Array index must be an Int, but got {:?}", loc, index_type));
                }
                match array_type {
                    Type::QuantumRegister(size_opt) => {
                        if let (Some(size), ASTNode::IntLiteral(idx)) = (size_opt, &**index) {
                            if *idx < 0 || *idx as usize >= size {
                                return Err(format!("Type Error at {}: Qubit index {} is out of bounds for register of size {}.", loc, idx, size));
                            }
                        }
                        Ok(Type::Qubit)
                    }
                    Type::Array(inner_type) => Ok(*inner_type),
                    Type::String => Ok(Type::String),
                    Type::Dict => Ok(Type::Any),
                    _ => Err(format!("Type Error at {}: Cannot perform array access '[]' on type {:?}", loc, array_type)),
                }
            }

            ASTNode::MemberAccess { object, member } => {
                let object_type = Self::check(object, env, Option::None)?;

                if let Type::Module(module_types) = object_type {
                    match module_types.get(member) {
                        Some(t) => Ok(t.clone()),
                        None => Err(format!("Type Error: Module has no member named '{}'", member))
                    }
                }

                else if member == "length" {
                    match object_type {
                        Type::Array(_) | Type::String | Type::Dict => Ok(Type::Int),
                        Type::QuantumRegister(Some(_size)) => Ok(Type::Int),
                        Type::QuantumRegister(None) => Ok(Type::Int),
                        _ => Err(format!("Type Error: Cannot get .length of type {:?}", object_type)),
                    }
                } else {
                    if let Type::Dict = object_type {
                        Ok(Type::Any)
                    } else {
                        Err(format!("Type Error: Type {:?} has no member named '{}'", object_type, member))
                    }
                }
            }
            ASTNode::Dagger { .. } => {
                Self::check_gate_expression(node, env)
            }


            ASTNode::Apply { gate_expr, arguments, loc } => {

                let gate_type = Self::check_gate_expression(gate_expr, env)?;


                match gate_type {
                    Type::Function(param_types, return_type) => {

                        if arguments.len() != param_types.len() {
                            return Err(format!(
                                "Type Error at {}: Gate requires {} arguments, but got {}",
                                loc, param_types.len(), arguments.len()
                            ));
                        }

                        for (i, arg_node) in arguments.iter().enumerate() {
                            let arg_type = Self::check(arg_node, env, Option::None)?;
                            let expected_type = &param_types[i];

                            if arg_type != *expected_type && *expected_type != Type::Any {
                                return Err(format!(
                                    "Type Error at {}: Argument {} is wrong type. Expected {:?}, got {:?}",
                                    loc, (i + 1), expected_type, arg_type
                                ));
                            }
                        }

                        Ok(*return_type)
                    }
                    _ => Err(format!("Type Error at {}: This expression is not a callable gate.", loc))
                }
            }


            ASTNode::Gate { .. } => {
                Self::check_gate_expression(node, env)
            }


            ASTNode::Controlled { gate_expr, loc } => {

                Self::check_gate_expression(node, env)
            }

            ASTNode::Measure(target_expr) => {
                let target_type = Self::check(target_expr, env, Option::None)?;
                match target_type {
                    Type::Qubit | Type::Any => Ok(Type::Int),
                    _ => Err(format!("Type Error: 'measure' can only be used on a single Qubit, got {:?}", target_type)),
                }
            }

            ASTNode::Import { path, alias } => {
                let module_types = Self::check_module(path)?;
                let info = Self::immutable_info(Type::Module(module_types));
                env.borrow_mut().set(alias.clone(), info);
                Ok(Type::None)
            }

            ASTNode::FromImport { path, spec } => {
                let module_types = Self::check_module(path)?;
                match spec {
                    ImportSpec::All => {
                        for (name, var_type) in module_types {
                            env.borrow_mut().set(name, Self::immutable_info(var_type));
                        }
                    }
                    ImportSpec::List(names) => {
                        for name in names {
                            if let Some(var_type) = module_types.get(name.as_str()) {
                                env.borrow_mut().set(name.clone(), Self::immutable_info(var_type.clone()));
                            } else {
                                return Err(format!("Type Error: Cannot import name '{}'. It does not exist in module.", name));
                            }
                        }
                    }
                }
                Ok(Type::None)
            }


            ASTNode::FunctionDeclaration { name, parameters, return_type, body, .. } => {
                let param_types: Vec<Type> = parameters.iter().map(|p| p.param_type.clone()).collect();
                let rt = return_type.clone().unwrap_or(Type::Any);
                let func_type = Type::Function(param_types, Box::new(rt.clone()));
                env.borrow_mut().set(name.clone(), Self::immutable_info(func_type));
                let func_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));
                for param in parameters {
                    func_env.borrow_mut().set(param.name.clone(), Self::immutable_info(param.param_type.clone()));
                }
                Self::check(body, &func_env, Some(&rt))?;
                Ok(Type::None)
            }

            ASTNode::CircuitDeclaration { name, parameters, return_type, body, .. } => {
                let param_types: Vec<Type> = parameters.iter().map(|p| p.param_type.clone()).collect();
                let rt = return_type.clone().unwrap_or(Type::Any);
                let func_type = Type::Function(param_types, Box::new(rt.clone()));
                env.borrow_mut().set(name.clone(), Self::immutable_info(func_type));
                let func_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));
                for param in parameters {
                    func_env.borrow_mut().set(param.name.clone(), Self::immutable_info(param.param_type.clone()));
                }
                Self::check(body, &func_env, Some(&rt))?;
                Ok(Type::None)
            }

            ASTNode::Return(value_expr) => {
                let value_type = if let Some(expr) = value_expr {
                    Self::check(expr, env, Option::None)?
                } else {
                    Type::None
                };
               match expected_return_type {
                    Option::None => Err("Type Error: 'return' statement found outside of a function.".to_string()),
                    Some(expected) => {
                        if value_type != *expected && *expected != Type::Any {
                            Err(format!("Type Error: Function expected return type {:?}, but found return with type {:?}", expected, value_type))
                        } else {
                            Ok(Type::None)
                        }
                    }
                }
            }

            ASTNode::While { condition, body } => {
                let cond_type = Self::check(condition, env, Option::None)?;
                if cond_type != Type::Bool {
                    return Err(format!("Type Error: 'while' loop condition must be a Bool, but got {:?}", cond_type));
                }
                Self::check(body, env, expected_return_type)?;
                Ok(Type::None)
            }

            ASTNode::ArrayLiteral(elements) => {
                if elements.is_empty() {
                    Ok(Type::Array(Box::new(Type::Any)))
                } else {
                    let first_type = Self::check(&elements[0], env, Option::None)?;
                    for (i, element) in elements.iter().skip(1).enumerate() {
                        let element_type = Self::check(element, env, Option::None)?;
                        if element_type != first_type {
                            return Err(format!("Type Error: Array literal has mismatched types. Element 0 has type {:?}, but element {} has type {:?}", first_type, (i+1), element_type));
                        }
                    }
                    Ok(Type::Array(Box::new(first_type)))
                }
            }

            ASTNode::Range { start, end, .. } => {
                let start_type = Self::check(start, env, Option::None)?;
                let end_type = Self::check(end, env, Option::None)?;

                if start_type != Type::Int {
                    return Err(format!("Type Error: Range start must be an Int, got {:?}", start_type));
                }
                if end_type != Type::Int {
                    return Err(format!("Type Error: Range end must be an Int, got {:?}", end_type));
                }


                Ok(Type::Custom("range".to_string()))
            }

            ASTNode::For { variable, iterator, body } => {
                let iterator_type = Self::check(iterator, env, Option::None)?;
                let element_type = match iterator_type {
                    Type::Array(inner_type) => *inner_type,
                    Type::String => Type::String,
                    Type::Dict => Type::String,
                    Type::Custom(name) if name == "range" => Type::Int,
                    Type::Any => Type::Any,
                    _ => return Err(format!("Type Error: 'for' loop cannot iterate over type {:?}", iterator_type)),
                };
                let loop_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));
                loop_env.borrow_mut().set(variable.clone(), Self::immutable_info(element_type));
                Self::check(body, &loop_env, expected_return_type)?;
                Ok(Type::None)
            }

            ASTNode::If { condition, then_block, elif_blocks, else_block } => {
                let cond_type = Self::check(condition, env, Option::None)?;
                if cond_type != Type::Bool {
                    return Err(format!("Type Error: 'if' condition must be a Bool, but got {:?}", cond_type));
                }
                let then_type = Self::check(then_block, env, expected_return_type)?;
                for (elif_cond, elif_body) in elif_blocks {
                    let elif_cond_type = Self::check(elif_cond, env, Option::None)?;
                    if elif_cond_type != Type::Bool {
                        return Err(format!("Type Error: 'elif' condition must be a Bool, but got {:?}", elif_cond_type));
                    }
                    Self::check(elif_body, env, expected_return_type)?;
                }
                if let Some(else_body) = else_block {
                    Self::check(else_body, env, expected_return_type)?;
                }
                Ok(then_type)
            }

            ASTNode::Block(statements) => {
                for stmt in statements {
                    Self::check(stmt, env, expected_return_type)?;
                }
                Ok(Type::None)
            }

            ASTNode::TryCatch { try_block, catch_block, .. } => {
                Self::check(try_block, env, expected_return_type)?;
                Self::check(catch_block, env, expected_return_type)?;
                Ok(Type::None)
            }

            _ => {
                Err(format!("Type checking is not implemented for this node: {:?}", node))
            }
        }
    }

}
