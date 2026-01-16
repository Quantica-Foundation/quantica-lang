/* src/type_checker.rs */

use crate::error::{find_similar_names, levenshtein_distance};
use crate::lexer::Lexer;
use crate::parser::ast::ImportPath;
use crate::parser::ast::Loc;
use crate::parser::ast::{ASTNode, BinaryOperator, ImportSpec, Type, UnaryOperator};
use crate::parser::Parser;
use crate::qubit_lifecycle::{QubitId, QubitLifecycleManager, QubitOperation, QubitState};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;
use std::string::String;

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
        TypeEnvironment {
            store: HashMap::new(),
            outer: Option::None,
        }
    }

    pub fn new_enclosed(outer_env: Rc<RefCell<TypeEnvironment>>) -> Self {
        TypeEnvironment {
            store: HashMap::new(),
            outer: Some(outer_env),
        }
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

pub struct TypeChecker {
    lifecycle_manager: QubitLifecycleManager,
}

impl TypeChecker {
    pub fn prefill_environment(env: &Rc<RefCell<TypeEnvironment>>) {
        let mut env_mut = env.borrow_mut();

        let immut = |t: Type| TypeInfo {
            var_type: t,
            is_mutable: false,
        };

        let any = Type::Any;
        let none = Type::None;
        let qubit_type = Type::Qubit;
        let none_type = Box::new(none.clone());

        env_mut.set(
            "print".to_string(),
            immut(Type::Function(vec![], Box::new(none.clone()))),
        );
        env_mut.set(
            "input".to_string(),
            immut(Type::Function(vec![Type::Any], Box::new(Type::String))),
        );
        env_mut.set(
            "split".to_string(),
            immut(Type::Function(
                vec![Type::String, Type::String],
                Box::new(Type::Array(Box::new(Type::String))),
            )),
        );
        env_mut.set(
            "debug_state".to_string(),
            immut(Type::Function(
                vec![Type::QuantumRegister(None)],
                none_type.clone(),
            )),
        );
        env_mut.set(
            "assert".to_string(),
            immut(Type::Function(
                vec![Type::Bool, Type::String],
                none_type.clone(),
            )),
        );
        env_mut.set(
            "maybe".to_string(),
            immut(Type::Function(
                vec![Type::Any, Type::Float],
                Box::new(Type::Any),
            )),
        );
        env_mut.set(
            "sample".to_string(),
            immut(Type::Function(vec![Type::Any], Box::new(Type::Any))),
        );
        env_mut.set(
            "echo".to_string(),
            immut(Type::Function(vec![Type::Any], Box::new(Type::Any))),
        );
        env_mut.set(
            "len".to_string(),
            immut(Type::Function(vec![any.clone()], Box::new(Type::Int))),
        );
        env_mut.set(
            "type_of".to_string(),
            immut(Type::Function(vec![any.clone()], Box::new(Type::String))),
        );
        env_mut.set(
            "to_string".to_string(),
            immut(Type::Function(vec![any.clone()], Box::new(Type::String))),
        );
        env_mut.set(
            "to_int".to_string(),
            immut(Type::Function(vec![any.clone()], Box::new(Type::Int))),
        );
        env_mut.set(
            "to_float".to_string(),
            immut(Type::Function(vec![any.clone()], Box::new(Type::Float))),
        );

        let single_qubit_gate = Type::Function(vec![qubit_type.clone()], none_type.clone());
        env_mut.set("hadamard".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("x".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("y".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("z".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("s".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("t".to_string(), immut(single_qubit_gate.clone()));
        env_mut.set("reset".to_string(), immut(single_qubit_gate.clone()));

        let two_qubit_gate = Type::Function(
            vec![qubit_type.clone(), qubit_type.clone()],
            none_type.clone(),
        );
        env_mut.set("cnot".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("swap".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("cz".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("cs".to_string(), immut(two_qubit_gate.clone()));
        env_mut.set("ct".to_string(), immut(two_qubit_gate.clone()));

        let three_qubit_gate = Type::Function(
            vec![qubit_type.clone(), qubit_type.clone(), qubit_type.clone()],
            none_type.clone(),
        );
        env_mut.set("ccx".to_string(), immut(three_qubit_gate.clone()));
        env_mut.set("toffoli".to_string(), immut(three_qubit_gate));

        let cphase_gate = Type::Function(
            vec![Type::Float, qubit_type.clone(), qubit_type.clone()],
            none_type.clone(),
        );
        env_mut.set("cphase".to_string(), immut(cphase_gate));

        let u_gate = Type::Function(
            vec![Type::Float, Type::Float, Type::Float, qubit_type.clone()],
            none_type.clone(),
        );
        env_mut.set("u".to_string(), immut(u_gate));

        let parameterized_gate =
            Type::Function(vec![Type::Float, qubit_type.clone()], none_type.clone());
        env_mut.set("rx".to_string(), immut(parameterized_gate.clone()));
        env_mut.set("ry".to_string(), immut(parameterized_gate.clone()));
        env_mut.set("rz".to_string(), immut(parameterized_gate.clone()));

        env_mut.set(
            "_graphics_create_canvas".to_string(),
            immut(Type::Function(
                vec![Type::Int, Type::Int],
                Box::new(Type::Int),
            )),
        );

        env_mut.set(
            "_graphics_set_background".to_string(),
            immut(Type::Function(
                vec![Type::Int, Type::Int, Type::Int, Type::Int, Type::Int],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "_graphics_clear".to_string(),
            immut(Type::Function(vec![Type::Int], Box::new(Type::None))),
        );

        env_mut.set(
            "_graphics_draw_line".to_string(),
            immut(Type::Function(
                vec![
                    Type::Int,
                    Type::Float,
                    Type::Float,
                    Type::Float,
                    Type::Float,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Float,
                ],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "_graphics_draw_rect".to_string(),
            immut(Type::Function(
                vec![
                    Type::Int,
                    Type::Float,
                    Type::Float,
                    Type::Float,
                    Type::Float,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                ],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "_graphics_draw_circle".to_string(),
            immut(Type::Function(
                vec![
                    Type::Int,
                    Type::Float,
                    Type::Float,
                    Type::Float,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                ],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "_graphics_draw_text".to_string(),
            immut(Type::Function(
                vec![
                    Type::Int,
                    Type::Float,
                    Type::Float,
                    Type::String,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Int,
                    Type::Float,
                ],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "_graphics_save_svg".to_string(),
            immut(Type::Function(
                vec![Type::Int, Type::String],
                Box::new(Type::Int),
            )),
        );

        env_mut.set(
            "_graphics_save_png".to_string(),
            immut(Type::Function(
                vec![Type::Int, Type::String],
                Box::new(Type::Int),
            )),
        );

        env_mut.set(
            "_graphics_destroy_canvas".to_string(),
            immut(Type::Function(vec![Type::Int], Box::new(Type::None))),
        );

        env_mut.set(
            "_graphics_create_plot".to_string(),
            immut(Type::Function(vec![Type::Int], Box::new(Type::Int))),
        );

        env_mut.set(
            "_graphics_plot_set_data".to_string(),
            immut(Type::Function(
                vec![
                    Type::Int,
                    Type::Array(Box::new(Type::Float)),
                    Type::Array(Box::new(Type::Float)),
                    Type::Int,
                ],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "_graphics_plot_set_title".to_string(),
            immut(Type::Function(
                vec![Type::Int, Type::String],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "_graphics_plot_render".to_string(),
            immut(Type::Function(
                vec![Type::Int, Type::Int],
                Box::new(Type::Int),
            )),
        );

        env_mut.set(
            "_graphics_destroy_plot".to_string(),
            immut(Type::Function(vec![Type::Int], Box::new(Type::None))),
        );

        env_mut.set(
            "file_write".to_string(),
            immut(Type::Function(
                vec![Type::String, Type::String],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "file_read".to_string(),
            immut(Type::Function(
                vec![Type::String],
                Box::new(Type::String),
            )),
        );

        env_mut.set(
            "time".to_string(),
            immut(Type::Function(
                vec![],
                Box::new(Type::Float),
            )),
        );

        env_mut.set(
            "matrix_update".to_string(),
            immut(Type::Function(
                vec![
                    Type::Array(Box::new(Type::Array(Box::new(Type::Float)))),
                    Type::Array(Box::new(Type::Float)),
                    Type::Array(Box::new(Type::Float)),
                    Type::Float,
                    Type::Int,
                    Type::Int
                ],
                Box::new(Type::None),
            )),
        );

        env_mut.set(
            "compute_input_gradient".to_string(),
            immut(Type::Function(
                vec![
                    Type::Array(Box::new(Type::Array(Box::new(Type::Float)))),
                    Type::Array(Box::new(Type::Float)),
                    Type::Int,
                    Type::Int
                ],
                Box::new(Type::Array(Box::new(Type::Float))),
            )),
        );

        env_mut.set(
            "file_append".to_string(),
            immut(Type::Function(
                vec![Type::String, Type::String],
                Box::new(Type::None),
            )),
        );

    }

    pub fn check_program(node: &ASTNode) -> Result<(), String> {
        if let ASTNode::Program(statements) = node {

            let env = Rc::new(RefCell::new(TypeEnvironment::new()));
            let mut lifecycle = QubitLifecycleManager::new(true);


            Self::prefill_environment(&env);


            {
                let mut env_mut = env.borrow_mut();


                env_mut.set(
                    "file_read".to_string(),
                    TypeInfo {
                        var_type: Type::Function(
                            vec![Type::String],
                            Box::new(Type::String)
                        ),
                        is_mutable: false
                    }
                );
                env_mut.set(
                    "file_append".to_string(),
                    TypeInfo {
                        var_type: Type::Function(
                            vec![Type::String, Type::String],
                            Box::new(Type::None)
                        ),
                        is_mutable: false
                    }
                );


                env_mut.set(
                    "file_write".to_string(),
                    TypeInfo {
                        var_type: Type::Function(
                            vec![Type::String, Type::String],
                            Box::new(Type::None)
                        ),
                        is_mutable: false
                    }
                );
            }


            for stmt in statements {
                match stmt {
                    ASTNode::FunctionDeclaration { name, parameters, return_type, .. } => {
                        let param_types: Vec<Type> = parameters.iter().map(|p| p.param_type.clone()).collect();
                        let rt = return_type.clone().unwrap_or(Type::Any);
                        let func_type = Type::Function(param_types, Box::new(rt));

                        env.borrow_mut().set(name.clone(), TypeInfo {
                            var_type: func_type,
                            is_mutable: false
                        });
                    },
                    ASTNode::CircuitDeclaration { name, parameters, return_type, .. } => {
                         let param_types: Vec<Type> = parameters.iter().map(|p| p.param_type.clone()).collect();
                         let rt = return_type.clone().unwrap_or(Type::Any);
                         let func_type = Type::Function(param_types, Box::new(rt));

                         env.borrow_mut().set(name.clone(), TypeInfo {
                             var_type: func_type,
                             is_mutable: false
                         });
                    },
                    ASTNode::ClassDeclaration { name, .. } => {
                        let class_type = Type::Class(name.clone());
                        env.borrow_mut().set(name.clone(), TypeInfo {
                            var_type: class_type,
                            is_mutable: false
                        });
                    },
                    _ => {}
                }
            }


            for stmt in statements {
                Self::check_with_lifecycle(stmt, &env, None, &mut lifecycle)?;
            }

            Ok(())
        } else {
            Err("Expected Program node".to_string())
        }
    }

    fn check_with_lifecycle(
        node: &ASTNode,
        env: &Rc<RefCell<TypeEnvironment>>,
        expected_return_type: Option<&Type>,
        lifecycle: &mut QubitLifecycleManager,
    ) -> Result<Type, String> {
        match node {
            ASTNode::QuantumDeclaration {
                name,
                size,
                initial_state,
            } => {
                let register_size = if let Some(size_expr) = size {
                    if let ASTNode::IntLiteral(n) = &**size_expr {
                        *n as usize
                    } else {
                        1
                    }
                } else {
                    1
                };

                lifecycle.register_qubits(name, register_size, QubitState::Classical(false));

                Self::check(node, env, expected_return_type)
            }

            ASTNode::Apply {
                gate_expr,
                arguments,
                loc,
            } => {
                let qubit_ids = Self::extract_qubit_ids(arguments)?;
                let gate_name = Self::extract_gate_name(gate_expr)?;
                let is_controlled = Self::is_controlled_gate(gate_expr);

                if is_controlled {
                    let num_controls = Self::count_controls(gate_expr);
                    let (controls, targets) = qubit_ids.split_at(num_controls);
                    lifecycle
                        .record_controlled_gate(controls, targets, &gate_name, *loc)
                        .map_err(|e| e.to_string())?;
                } else {
                    for qubit_id in &qubit_ids {
                        lifecycle
                            .record_operation(
                                qubit_id,
                                QubitOperation::ApplyGate(gate_name.clone()),
                                *loc,
                            )
                            .map_err(|e| e.to_string())?;
                    }
                }

                Self::check(node, env, expected_return_type)
            }

            ASTNode::Measure(qubit_expr) => {
                let qubit_ids = Self::extract_qubit_ids(&[*qubit_expr.clone()])?;
                let loc = Self::get_node_location(qubit_expr);

                for qubit_id in &qubit_ids {
                    lifecycle
                        .record_operation(qubit_id, QubitOperation::Measure, loc)
                        .map_err(|e| e.to_string())?;
                }

                Self::check(node, env, expected_return_type)
            }

            ASTNode::LetDeclaration {
                name,
                type_annotation,
                value,
                is_mutable,
                ..
            } => {
                let value_type = Self::check_with_lifecycle(value, env, None, lifecycle)?;

                let final_type = match type_annotation {
                    Some(expected_type) => {
                        if value_type != *expected_type
                            && value_type != Type::None
                            && *expected_type != Type::Any
                            && value_type != Type::Any
                        {
                            return Err(format!(
                                "Type Error: Variable '{}' annotated as {:?} but got {:?}",
                                name, expected_type, value_type
                            ));
                        }
                        expected_type.clone()
                    }
                    None => value_type,
                };

                env.borrow_mut().set(
                    name.clone(),
                    TypeInfo {
                        var_type: final_type,
                        is_mutable: *is_mutable,
                    },
                );
                Ok(Type::None)
            }

            ASTNode::Block(statements) => {
                for stmt in statements {
                    Self::check_with_lifecycle(stmt, env, expected_return_type, lifecycle)?;
                }
                Ok(Type::None)
            }

            ASTNode::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                let cond_type = Self::check(condition, env, None)?;
                if cond_type != Type::Bool {
                    return Err(format!(
                        "Type Error: 'if' condition must be Bool, got {:?}",
                        cond_type
                    ));
                }

                let then_type =
                    Self::check_with_lifecycle(then_block, env, expected_return_type, lifecycle)?;

                for (elif_cond, elif_body) in elif_blocks {
                    let elif_cond_type = Self::check(elif_cond, env, None)?;
                    if elif_cond_type != Type::Bool {
                        return Err(format!(
                            "Type Error: 'elif' condition must be Bool, got {:?}",
                            elif_cond_type
                        ));
                    }
                    Self::check_with_lifecycle(elif_body, env, expected_return_type, lifecycle)?;
                }

                if let Some(else_body) = else_block {
                    Self::check_with_lifecycle(else_body, env, expected_return_type, lifecycle)?;
                }
                Ok(then_type)
            }

            ASTNode::While { condition, body } => {
                let cond_type = Self::check(condition, env, None)?;
                if cond_type != Type::Bool {
                    return Err(format!(
                        "Type Error: 'while' condition must be Bool, got {:?}",
                        cond_type
                    ));
                }
                Self::check_with_lifecycle(body, env, expected_return_type, lifecycle)?;
                Ok(Type::None)
            }

            ASTNode::For {
                variable,
                iterator,
                body,
            } => {
                let iterator_type = Self::check(iterator, env, None)?;
                let element_type = match iterator_type {
                    Type::Array(inner) => *inner,
                    Type::String => Type::String,
                    Type::Dict => Type::String,
                    Type::Custom(name) if name == "range" => Type::Int,
                    Type::Any => Type::Any,
                    _ => {
                        return Err(format!(
                            "Type Error: Cannot iterate over {:?}",
                            iterator_type
                        ))
                    }
                };

                let loop_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));
                loop_env.borrow_mut().set(
                    variable.clone(),
                    TypeInfo {
                        var_type: element_type,
                        is_mutable: false,
                    },
                );

                Self::check_with_lifecycle(body, &loop_env, expected_return_type, lifecycle)?;
                Ok(Type::None)
            }

            ASTNode::Return(value_expr) => {
                if let Some(expr) = value_expr {
                    let _ = Self::check_with_lifecycle(expr, env, None, lifecycle)?;
                }

                Self::check(node, env, expected_return_type)
            }

            _ => Self::check(node, env, expected_return_type),
        }
    }

    fn extract_qubit_ids(arguments: &[ASTNode]) -> Result<Vec<QubitId>, String> {
        let mut ids = Vec::new();

        for arg in arguments {
            match arg {
                ASTNode::ArrayAccess { array, index, .. } => {
                    if let ASTNode::Identifier { name, .. } = &**array {
                        if let ASTNode::IntLiteral(idx) = &**index {
                            ids.push(QubitId::new(name.clone(), *idx as usize));
                        }
                    }
                }
                ASTNode::Identifier { name, .. } => {
                    ids.push(QubitId::new(name.clone(), 0));
                }
                _ => {}
            }
        }

        Ok(ids)
    }

    fn extract_gate_name(gate_expr: &ASTNode) -> Result<String, String> {
        match gate_expr {
            ASTNode::Gate { name, .. } => Ok(name.clone()),
            ASTNode::ParameterizedGate { name, .. } => Ok(name.clone()),
            ASTNode::Controlled { gate_expr, .. } => Self::extract_gate_name(gate_expr),
            ASTNode::Dagger { gate_expr, .. } => Self::extract_gate_name(gate_expr),
            _ => Err("Invalid gate expression".to_string()),
        }
    }

    fn is_controlled_gate(gate_expr: &ASTNode) -> bool {
        matches!(gate_expr, ASTNode::Controlled { .. })
            || matches!(gate_expr, ASTNode::Dagger { gate_expr: inner, .. }
            if Self::is_controlled_gate(inner))
    }

    fn count_controls(gate_expr: &ASTNode) -> usize {
        match gate_expr {
            ASTNode::Controlled { gate_expr, .. } => 1 + Self::count_controls(gate_expr),
            ASTNode::Dagger { gate_expr, .. } => Self::count_controls(gate_expr),
            _ => 0,
        }
    }

    fn get_node_location(node: &ASTNode) -> Loc {
        match node {
            ASTNode::ArrayAccess { loc, .. } => *loc,
            ASTNode::Identifier { loc, .. } => *loc,
            _ => Loc { line: 0, column: 0 },
        }
    }

    fn immutable_info(t: Type) -> TypeInfo {
        TypeInfo {
            var_type: t,
            is_mutable: false,
        }
    }

    fn check_module(path: &ImportPath) -> Result<HashMap<String, Type>, String> {
        let raw_path = match path {
            ImportPath::File(f) => {
                if f.ends_with(".qc") || f.contains('/') || f.contains('\\') {
                    f.clone()
                } else {
                    format!("q_packages/{}/init.qc", f)
                }
            }
            ImportPath::Module(m) => m.join("/") + ".qc",
        };

        let mut final_path = std::path::PathBuf::from(&raw_path);

        if !final_path.exists() {
            if let Ok(mut exe_path) = std::env::current_exe() {
                exe_path.pop();
                let install_path = exe_path.join(&raw_path);
                if install_path.exists() {
                    final_path = install_path;
                }
            }
        }

        let source = fs::read_to_string(&final_path).map_err(|e| {
            format!(
                "Type Check Error: Failed to read module '{}': {}",
                raw_path, e
            )
        })?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Module Lexer Error: {}", e))?;
        let mut parser = Parser::new(tokens);
        let ast = parser
            .parse()
            .map_err(|e| format!("Module Parser Error: {}", e))?;

        let module_env = Rc::new(RefCell::new(TypeEnvironment::new()));
        Self::prefill_environment(&module_env);

        if let ASTNode::Program(statements) = ast {
            for stmt in &statements {
                match stmt {
                    ASTNode::FunctionDeclaration {
                        name,
                        parameters,
                        return_type,
                        ..
                    } => {
                        let param_types: Vec<Type> =
                            parameters.iter().map(|p| p.param_type.clone()).collect();
                        let rt = return_type.clone().unwrap_or(Type::Any);
                        let func_type = Type::Function(param_types, Box::new(rt));

                        module_env.borrow_mut().set(
                            name.clone(),
                            TypeInfo {
                                var_type: func_type,
                                is_mutable: false,
                            },
                        );
                    }
                    ASTNode::CircuitDeclaration {
                        name,
                        parameters,
                        return_type,
                        ..
                    } => {
                        let param_types: Vec<Type> =
                            parameters.iter().map(|p| p.param_type.clone()).collect();
                        let rt = return_type.clone().unwrap_or(Type::Any);
                        let func_type = Type::Function(param_types, Box::new(rt));

                        module_env.borrow_mut().set(
                            name.clone(),
                            TypeInfo {
                                var_type: func_type,
                                is_mutable: false,
                            },
                        );
                    }
                    ASTNode::ClassDeclaration { name, .. } => {
                        let class_type = Type::Class(name.clone());
                        module_env.borrow_mut().set(
                            name.clone(),
                            TypeInfo {
                                var_type: class_type,
                                is_mutable: false,
                            },
                        );

                        /* Self::check(stmt, &module_env, None)?; */
                    }
                    _ => {}
                }
            }

            for stmt in statements {
                Self::check(&stmt, &module_env, None)?;
            }
        } else {
            return Err("Module root is not a Program node".to_string());
        }

        let module_types = module_env
            .borrow()
            .store
            .iter()
            .map(|(k, v)| (k.clone(), v.var_type.clone()))
            .collect();

        Ok(module_types)
    }

    fn resolve_method(env: &Rc<RefCell<TypeEnvironment>>, method_key: &str) -> Option<Type> {
        if let Some(info) = env.borrow().get(method_key) {
            return Some(info.var_type.clone());
        }

        let mut current_env = Some(env.clone());

        while let Some(env_rc) = current_env {
            let env_ref = env_rc.borrow();

            for info in env_ref.store.values() {
                if let Type::Module(module_map) = &info.var_type {
                    if let Some(method_type) = module_map.get(method_key) {
                        return Some(method_type.clone());
                    }
                }
            }

            current_env = env_ref.outer.clone();
        }

        None
    }

    fn check_gate_expression(
        node: &ASTNode,
        env: &Rc<RefCell<TypeEnvironment>>,
    ) -> Result<Type, String> {
        let loc = match node {
            ASTNode::Gate { loc, .. }
            | ASTNode::ParameterizedGate { loc, .. }
            | ASTNode::Controlled { loc, .. }
            | ASTNode::Dagger { loc, .. } => *loc,
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

                Err(format!(
                    "Type Error at {}: Unknown quantum gate '{}'.",
                    loc, name
                ))
            }

            ASTNode::Dagger { gate_expr, .. } => {
                let inner_gate_type = Self::check_gate_expression(gate_expr, env)?;

                if let Type::Function(..) = inner_gate_type {
                    Ok(inner_gate_type)
                } else {
                    Err(format!(
                        "Type Error at {}: 'dagger' can only be applied to a gate or circuit.",
                        loc
                    ))
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
                    _ => Err(format!(
                        "Type Error at {}: 'controlled' can only be applied to a callable gate.",
                        loc
                    )),
                }
            }

            ASTNode::ParameterizedGate {
                name,
                parameters,
                loc,
            } => {
                let gate_name_lower = name.to_lowercase();

                let gate_info = env.borrow().get(&gate_name_lower).ok_or_else(|| {
                    format!(
                        "Type Error at {}: Unknown parameterized gate '{}'.",
                        loc, name
                    )
                })?;

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

                            if param_type != *expected_type
                                && !(*expected_type == Type::Float && param_type == Type::Int)
                            {
                                return Err(format!(
                                "Type Error at {}: Parameter {} of gate '{}' has wrong type. Expected {:?}, got {:?}",
                                loc, i + 1, name, expected_type, param_type
                            ));
                            }
                        }

                        let remaining_params = param_types[parameters.len()..].to_vec();
                        Ok(Type::Function(remaining_params, return_type))
                    }
                    _ => Err(format!(
                        "Type Error at {}: '{}' is not a parameterized gate.",
                        loc, name
                    )),
                }
            }

            _ => Err(format!("Type Error: This expression is not a valid gate.")),
        }
    }

    pub fn check(
        node: &ASTNode,
        env: &Rc<RefCell<TypeEnvironment>>,
        expected_return_type: Option<&Type>,
    ) -> Result<Type, String> {
        match node {
            ASTNode::IntLiteral(_) => Ok(Type::Int),
            ASTNode::FloatLiteral(_) => Ok(Type::Float),
            ASTNode::StringLiteral(_) => Ok(Type::String),
            ASTNode::BoolLiteral(_) => Ok(Type::Bool),
            ASTNode::NoneLiteral => Ok(Type::None),
            ASTNode::DictLiteral(_) => Ok(Type::Dict),
            ASTNode::QuantumKet(_) => Ok(Type::QuantumRegister(Some(1))),
            ASTNode::QuantumBra(_) => Err("Bra notation is not yet supported.".to_string()),

            ASTNode::LetDeclaration {
                name,
                type_annotation,
                value,
                is_mutable,
                ..
            } => {
                let value_type = Self::check(value, env, Option::None)?;

                let final_type = match type_annotation {
                    Some(expected_type) => {
                        if value_type != *expected_type
                            && value_type != Type::None
                            && *expected_type != Type::Any
                            && value_type != Type::Any
                        {
                            let is_quantum_compat = matches!(
                                (&value_type, expected_type),
                                (Type::QuantumRegister(_), Type::QuantumRegister(None))
                            );

                            let is_class_compat = match (&value_type, expected_type) {
                                (Type::Instance(got), Type::Custom(expected)) => got == expected,
                                _ => false,
                            };

                            if !is_quantum_compat && !is_class_compat {
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

                let info = TypeInfo {
                    var_type: final_type,
                    is_mutable: *is_mutable,
                };
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


                        if original_info.var_type != new_type
                            && original_info.var_type != Type::Any
                            && new_type != Type::None
                            && new_type != Type::Any
                        {
                            let is_quantum_compat = matches!(
                                (&new_type, &original_info.var_type),
                                (Type::QuantumRegister(_), Type::QuantumRegister(None))
                            );

                            let is_class_compat = match (&new_type, &original_info.var_type) {
                                (Type::Instance(got), Type::Custom(expected)) => got == expected,
                                _ => false,
                            };


                            let is_array_compat = match (&original_info.var_type, &new_type) {
                                (Type::Array(t1), Type::Array(t2)) => **t1 == Type::Any || **t2 == Type::Any,
                                _ => false
                            };


                            if !is_quantum_compat && !is_class_compat && !is_array_compat {
                                return Err(format!(
                                    "Type Error: Mismatched types in assignment. Cannot assign type {:?} to variable '{}' of type {:?}",
                                    new_type, name, original_info.var_type
                                ));
                            }
                        }



                        if let Type::Array(inner) = &original_info.var_type {
                            if **inner == Type::Any {
                                env.borrow_mut().set(name.clone(), TypeInfo {
                                    var_type: new_type,
                                    is_mutable: true
                                });
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
                    ASTNode::MemberAccess { object, member } => {
                        let object_type = Self::check(object, env, Option::None)?;

                        let class_name_opt = match &object_type {
                            Type::Instance(name) | Type::Custom(name) => Some(name.clone()),
                            _ => None,
                        };

                        if let Some(mut class_name) = class_name_opt {

                            if class_name.contains('.') {
                                if let Some(real_name) = class_name.split('.').last() {
                                    class_name = real_name.to_string();
                                }
                            }

                            let field_key = format!("{}::{}", class_name, member);


                            if let Some(field_info) = env.borrow().get(&field_key) {
                                return Ok(field_info.var_type.clone());
                            }



                            let mut found_type = None;
                            let mut current_env = Some(env.clone());

                            while let Some(env_rc) = current_env {
                                let env_ref = env_rc.borrow();
                                for info in env_ref.store.values() {
                                    if let Type::Module(mod_types) = &info.var_type {
                                        if let Some(field_type) = mod_types.get(&field_key) {
                                            found_type = Some(field_type.clone());
                                            break;
                                        }
                                    }
                                }
                                if found_type.is_some() {
                                    break;
                                }
                                current_env = env_ref.outer.clone();
                            }

                            if let Some(t) = found_type {
                                return Ok(t);
                            }

                            return Err(format!(
                                "Type Error: Class '{}' has no member named '{}'",
                                class_name, member
                            ));
                        }

                        if let Type::Module(module_types) = object_type {
                            match module_types.get(member) {
                                Some(t) => Ok(t.clone()),
                                None => Err(format!(
                                    "Type Error: Module has no member named '{}'",
                                    member
                                )),
                            }
                        } else if member == "length" {
                            match object_type {
                                Type::Array(_) | Type::String | Type::Dict => Ok(Type::Int),
                                Type::QuantumRegister(Some(_size)) => Ok(Type::Int),
                                Type::QuantumRegister(None) => Ok(Type::Int),
                                _ => Err(format!(
                                    "Type Error: Cannot get .length of type {:?}",
                                    object_type
                                )),
                            }
                        } else {
                            if let Type::Dict = object_type {
                                Ok(Type::Any)
                            } else {
                                Err(format!(
                                    "Type Error: Type {:?} has no member named '{}'",
                                    object_type, member
                                ))
                            }
                        }
                    }
                    _ => Err("Type Error: Assignment target must be an identifier or subscript expression.".to_string())
                }
            }
            ASTNode::Identifier { name, loc } => match env.borrow().get(name) {
                Some(info) => Ok(info.var_type.clone()),
                None => {
                    let mut all_names = Vec::new();
                    let mut current_env = Some(env.clone());

                    while let Some(env_rc) = current_env {
                        let env_ref = env_rc.borrow();
                        for var_name in env_ref.store.keys() {
                            all_names.push(var_name.clone());
                        }
                        current_env = env_ref.outer.clone();
                    }

                    let suggestions = find_similar_names(name, &all_names);

                    let mut error_msg = format!("Undefined variable '{}'", name);
                    if !suggestions.is_empty() {
                        error_msg.push_str(&format!(". Did you mean: {}?", suggestions.join(", ")));
                    }

                    Err(format!("Type Error at {}: {}", loc, error_msg))
                }
            },
            ASTNode::ClassDeclaration {
                name,
                fields,
                methods,
                constructor,
                ..
            } => {
                let class_type = Type::Class(name.clone());
                env.borrow_mut().set(
                    name.clone(),
                    TypeInfo {
                        var_type: class_type,
                        is_mutable: false,
                    },
                );

                let class_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));

                for field in fields {
                    class_env.borrow_mut().set(
                        field.name.clone(),
                        TypeInfo {
                            var_type: field.field_type.clone(),
                            is_mutable: true,
                        },
                    );
                    let global_field_key = format!("{}::{}", name, field.name);
                    env.borrow_mut().set(
                        global_field_key,
                        TypeInfo {
                            var_type: field.field_type.clone(),
                            is_mutable: true,
                        },
                    );
                }

                for method in methods {
                    let method_param_types: Vec<Type> = method
                        .parameters
                        .iter()
                        .map(|p| p.param_type.clone())
                        .collect();
                    let method_return_type = method.return_type.clone().unwrap_or(Type::None);
                    let method_func_type =
                        Type::Function(method_param_types, Box::new(method_return_type));

                    let global_method_key = format!("{}::{}", name, method.name);
                    env.borrow_mut().set(
                        global_method_key,
                        TypeInfo {
                            var_type: method_func_type,
                            is_mutable: false,
                        },
                    );
                }

                if let Some(constructor_node) = constructor {
                    if let ASTNode::FunctionDeclaration {
                        body, parameters, ..
                    } = &**constructor_node
                    {
                        let constructor_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(
                            class_env.clone(),
                        )));

                        let param_types: Vec<Type> =
                            parameters.iter().map(|p| p.param_type.clone()).collect();
                        let ctor_type = Type::Function(param_types, Box::new(Type::None));
                        env.borrow_mut().set(
                            format!("{}::init", name),
                            TypeInfo {
                                var_type: ctor_type,
                                is_mutable: false,
                            },
                        );

                        constructor_env.borrow_mut().set(
                            "self".to_string(),
                            TypeInfo {
                                var_type: Type::Instance(name.clone()),
                                is_mutable: false,
                            },
                        );

                        for param in parameters {
                            constructor_env.borrow_mut().set(
                                param.name.clone(),
                                TypeInfo {
                                    var_type: param.param_type.clone(),
                                    is_mutable: false,
                                },
                            );
                        }
                        Self::check(body, &constructor_env, Some(&Type::None))?;
                    }
                }

                for method in methods {
                    let method_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(
                        class_env.clone(),
                    )));

                    method_env.borrow_mut().set(
                        "self".to_string(),
                        TypeInfo {
                            var_type: Type::Instance(name.clone()),
                            is_mutable: false,
                        },
                    );

                    for param in &method.parameters {
                        method_env.borrow_mut().set(
                            param.name.clone(),
                            TypeInfo {
                                var_type: param.param_type.clone(),
                                is_mutable: false,
                            },
                        );
                    }

                    let return_type = method.return_type.as_ref().unwrap_or(&Type::None);
                    Self::check(&method.body, &method_env, Some(return_type))?;
                }

                Ok(Type::None)
            }

            ASTNode::NewInstance {
                class_name,
                arguments,
                loc,
            } => {
                let parts: Vec<&str> = class_name.split('.').collect();

                let class_type = if parts.len() > 1 {
                    let module_name = parts[0];
                    let target_class = parts[1];

                    let module_info = env.borrow().get(module_name).ok_or_else(|| {
                        format!("Type Error at {}: Unknown module '{}'", loc, module_name)
                    })?;

                    if let Type::Module(mod_types) = &module_info.var_type {
                        mod_types.get(target_class).cloned().ok_or_else(|| {
                            format!(
                                "Type Error at {}: Module '{}' has no class '{}'",
                                loc, module_name, target_class
                            )
                        })?
                    } else {
                        return Err(format!(
                            "Type Error at {}: '{}' is not a module",
                            loc, module_name
                        ));
                    }
                } else {
                    env.borrow()
                        .get(class_name)
                        .map(|info| info.var_type.clone())
                        .ok_or_else(|| {
                            format!("Type Error at {}: Unknown class '{}'", loc, class_name)
                        })?
                };

                if !matches!(class_type, Type::Class(_)) {
                    return Err(format!(
                        "Type Error at {}: '{}' is not a class",
                        loc, class_name
                    ));
                }

                let ctor_params: Vec<Type> = if let Type::Class(real_name) = &class_type {
                    vec![]
                } else {
                    vec![]
                };

                for arg in arguments {
                    Self::check(arg, env, None)?;
                }

                if let Type::Class(name) = class_type {
                    Ok(Type::Instance(name))
                } else {
                    Ok(Type::Instance(class_name.clone()))
                }
            }

            ASTNode::MethodCall {
                object,
                method_name,
                arguments,
                loc,
            } => {
                let object_type = Self::check(object, env, None)?;

                match object_type {
                    Type::Instance(class_name) | Type::Custom(class_name) => {
                        let method_key = format!("{}::{}", class_name, method_name);

                        if let Some(method_type) = Self::resolve_method(env, &method_key) {
                            if let Type::Function(param_types, return_type) = method_type {
                                if arguments.len() != param_types.len() {
                                    return Err(format!(
                                        "Type Error at {}: Method '{}.{}' expected {} arguments, but got {}",
                                        loc, class_name, method_name, param_types.len(), arguments.len()
                                    ));
                                }

                                for (i, arg_node) in arguments.iter().enumerate() {
                                    let arg_type = Self::check(arg_node, env, None)?;
                                    let expected_type = &param_types[i];

                                    if arg_type != *expected_type
                                        && *expected_type != Type::Any
                                        && arg_type != Type::Any
                                    {
                                        let is_quantum_compat = matches!(
                                            (&arg_type, expected_type),
                                            (Type::QuantumRegister(_), Type::QuantumRegister(None))
                                        );
                                        let is_class_compat = match (&arg_type, expected_type) {
                                            (Type::Instance(got), Type::Custom(expected)) => {
                                                got == expected ||
                                                got.ends_with(&format!(".{}", expected)) ||
                                                expected.ends_with(&format!(".{}", got))
                                            }
                                            (Type::Custom(got), Type::Custom(expected)) => {
                                                got == expected ||
                                                got.ends_with(&format!(".{}", expected)) ||
                                                expected.ends_with(&format!(".{}", got))
                                            }
                                            _ => false,
                                        };
                                        let is_dict_compat = match (&arg_type, expected_type) {
                                            (Type::Dict, Type::Custom(name)) if name == "Dict" => {
                                                true
                                            }
                                            (Type::Custom(name), Type::Dict) if name == "Dict" => {
                                                true
                                            }
                                            _ => false,
                                        };

                                        if !is_quantum_compat && !is_class_compat && !is_dict_compat
                                        {
                                            return Err(format!(
                                                "Type Error at {}: Argument {} of '{}.{}' is wrong type. Expected {:?}, got {:?}",
                                                loc, (i + 1), class_name, method_name, expected_type, arg_type
                                            ));
                                        }
                                    }
                                }

                                Ok(*return_type)
                            } else {
                                Ok(Type::Any)
                            }
                        } else {
                            return Err(format!(
                                "Type Error at {}: Class '{}' has no method named '{}'",
                                loc, class_name, method_name
                            ));
                        }
                    }

                    Type::Module(module_types) => {
                        if let Some(member_type) = module_types.get(method_name) {
                            match member_type {
                                Type::Function(param_types, return_type) => {
                                    if arguments.len() != param_types.len() {
                                        return Err(format!(
                                            "Type Error at {}: Module function '{}.{}' expected {} arguments, but got {}",
                                            loc, "module", method_name, param_types.len(), arguments.len()
                                        ));
                                    }

                                    for (i, arg_node) in arguments.iter().enumerate() {
                                        let arg_type = Self::check(arg_node, env, Option::None)?;
                                        let expected_type = &param_types[i];

                                        if arg_type != *expected_type
                                            && *expected_type != Type::Any
                                            && arg_type != Type::Any
                                        {
                                            let is_quantum_compat = matches!(
                                                (&arg_type, expected_type),
                                                (
                                                    Type::QuantumRegister(_),
                                                    Type::QuantumRegister(None)
                                                )
                                            );
                                            let is_class_compat = match (&arg_type, expected_type) {
                                                (Type::Instance(got), Type::Custom(expected)) => {
                                                    got == expected ||
                                                    got.ends_with(&format!(".{}", expected)) ||
                                                    expected.ends_with(&format!(".{}", got))
                                                }
                                                (Type::Custom(got), Type::Custom(expected)) => {
                                                    got == expected ||
                                                    got.ends_with(&format!(".{}", expected)) ||
                                                    expected.ends_with(&format!(".{}", got))
                                                }
                                                _ => false,
                                            };
                                            let is_dict_compat = match (&arg_type, expected_type) {
                                                (Type::Dict, Type::Custom(name))
                                                    if name == "Dict" =>
                                                {
                                                    true
                                                }
                                                (Type::Custom(name), Type::Dict)
                                                    if name == "Dict" =>
                                                {
                                                    true
                                                }
                                                _ => false,
                                            };
                                            let is_func_compat = match (&arg_type, expected_type) {
                                                (Type::Function(..), Type::Custom(name))
                                                    if name == "Function" =>
                                                {
                                                    true
                                                }
                                                _ => false,
                                            };

                                            if !is_quantum_compat
                                                && !is_class_compat
                                                && !is_dict_compat
                                                && !is_func_compat
                                            {
                                                return Err(format!(
                                                    "Type Error at {}: Argument {} of '{}.{}' is wrong type. Expected {:?}, got {:?}",
                                                    loc, (i + 1), "module", method_name, expected_type, arg_type
                                                ));
                                            }
                                        }
                                    }
                                    Ok(*return_type.clone())
                                }

                                Type::Class(class_name) => {
                                    let init_key = format!("{}::init", class_name);
                                    let constructor_params =
                                        if let Some(ctor_type) = module_types.get(&init_key) {
                                            if let Type::Function(params, _) = ctor_type {
                                                params.clone()
                                            } else {
                                                vec![]
                                            }
                                        } else {
                                            vec![]
                                        };

                                    if arguments.len() != constructor_params.len() {
                                        return Err(format!(
                                            "Type Error at {}: Constructor for '{}.{}' expected {} arguments, but got {}",
                                            loc, "module", method_name, constructor_params.len(), arguments.len()
                                        ));
                                    }

                                    for (i, arg_node) in arguments.iter().enumerate() {
                                        let arg_type = Self::check(arg_node, env, Option::None)?;
                                        let expected_type = &constructor_params[i];

                                        if arg_type != *expected_type
                                            && *expected_type != Type::Any
                                            && arg_type != Type::Any
                                        {
                                            let is_quantum_compat = matches!(
                                                (&arg_type, expected_type),
                                                (
                                                    Type::QuantumRegister(_),
                                                    Type::QuantumRegister(None)
                                                )
                                            );
                                            let is_class_compat = match (&arg_type, expected_type) {
                                                (Type::Instance(got), Type::Custom(expected)) => {
                                                    got == expected
                                                }
                                                _ => false,
                                            };
                                            let is_dict_compat = match (&arg_type, expected_type) {
                                                (Type::Dict, Type::Custom(name))
                                                    if name == "Dict" =>
                                                {
                                                    true
                                                }
                                                (Type::Custom(name), Type::Dict)
                                                    if name == "Dict" =>
                                                {
                                                    true
                                                }
                                                _ => false,
                                            };

                                            if !is_quantum_compat
                                                && !is_class_compat
                                                && !is_dict_compat
                                            {
                                                return Err(format!(
                                                    "Type Error at {}: Argument {} of constructor is wrong type. Expected {:?}, got {:?}",
                                                    loc, (i + 1), expected_type, arg_type
                                                ));
                                            }
                                        }
                                    }

                                    Ok(Type::Instance(class_name.clone()))
                                }

                                _ => Err(format!(
                                    "Type Error at {}: '{}.{}' is not a function or class.",
                                    loc, "module", method_name
                                )),
                            }
                        } else {
                            Err(format!(
                                "Type Error at {}: Module has no member named '{}'",
                                loc, method_name
                            ))
                        }
                    }

                    _ => Err(format!(
                        "Type Error at {}: Cannot call method '{}' on type {:?}",
                        loc, method_name, object_type
                    )),
                }
            }

            ASTNode::SelfRef { loc } => Err(format!(
                "Type Error at {}: 'self' can only be used inside class methods",
                loc
            )),

            ASTNode::FunctionCall {
                callee,
                arguments,
                loc,
                ..
            } => {
                let callee_type = Self::check(callee, env, Option::None)?;

                let name = format!("{:?}", callee);
                match callee_type {
                    Type::Class(class_name) => {
                        let init_key = format!("{}::init", class_name);
                        let constructor_params =
                            if let Some(ctor_info) = env.borrow().get(&init_key) {
                                if let Type::Function(params, _) = ctor_info.var_type {
                                    params
                                } else {
                                    vec![]
                                }
                            } else {
                                vec![]
                            };

                        if arguments.len() != constructor_params.len() {
                            return Err(format!(
                                "Type Error at {}: Constructor for '{}' expected {} arguments, but got {}",
                                loc, class_name, constructor_params.len(), arguments.len()
                            ));
                        }

                        for (i, arg_node) in arguments.iter().enumerate() {
                            let arg_type = Self::check(arg_node, env, Option::None)?;
                            let expected_type = &constructor_params[i];

                            if arg_type != *expected_type
                                && *expected_type != Type::Any
                                && arg_type != Type::Any
                            {
                                let is_quantum_compat = matches!(
                                    (&arg_type, expected_type),
                                    (Type::QuantumRegister(_), Type::QuantumRegister(None))
                                );

                                let is_class_compat = match (&arg_type, expected_type) {
                                    (Type::Instance(got), Type::Custom(expected)) => {
                                        got == expected
                                    }
                                    _ => false,
                                };

                                let is_dict_compat = match (&arg_type, expected_type) {
                                    (Type::Dict, Type::Custom(name)) if name == "Dict" => true,
                                    (Type::Custom(name), Type::Dict) if name == "Dict" => true,
                                    _ => false,
                                };

                                if !is_quantum_compat && !is_class_compat && !is_dict_compat {
                                    return Err(format!(
                                        "Type Error at {}: Argument {} of function call is wrong type. Expected {:?}, got {:?}",
                                        loc, (i + 1), expected_type, arg_type
                                    ));
                                }
                            }
                        }

                        Ok(Type::Instance(class_name))
                    }

                    Type::Custom(ref name) if name == "Function" => {
                        for arg in arguments {
                            Self::check(arg, env, Option::None)?;
                        }

                        Ok(Type::Any)
                    }

                    Type::Function(param_types, return_type) => {
                        if arguments.len() > 0 && param_types.len() == 0 {
                        } else if arguments.len() != param_types.len() {
                            return Err(format!(
                                "Type Error at {}: Function '{}' expected {} arguments, but got {}",
                                loc,
                                name,
                                param_types.len(),
                                arguments.len()
                            ));
                        }
                        for (i, arg_node) in arguments.iter().enumerate() {
                            if i >= param_types.len() {
                                break;
                            }
                            let arg_type = Self::check(arg_node, env, Option::None)?;
                            let expected_type = &param_types[i];

                            if arg_type != *expected_type
                                && *expected_type != Type::Any
                                && arg_type != Type::Any
                            {
                                let is_quantum_compat = matches!(
                                    (&arg_type, expected_type),
                                    (Type::QuantumRegister(_), Type::QuantumRegister(None))
                                );

                                let is_func_compat = match (&arg_type, expected_type) {
                                    (Type::Function(..), Type::Custom(name))
                                        if name == "Function" =>
                                    {
                                        true
                                    }
                                    _ => false,
                                };

                                let is_class_compat = match (&arg_type, expected_type) {
                                    (Type::Instance(got), Type::Custom(expected)) => {
                                        got == expected
                                    }
                                    _ => false,
                                };

                                if !is_quantum_compat && !is_class_compat {
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

            ASTNode::Binary {
                operator,
                left,
                right,
                loc,
            } => {
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
                        (Type::Array(t1), Type::Array(t2)) => {
                            if t1 == t2 {
                                return Ok(*t1);
                            } else {
                                return Ok(Type::Float);
                            }
                        }
                        _ => return Err(format!(
                            "Type Error at {}: Tensor product '***' is only defined for quantum registers and arrays, got {:?} and {:?}",
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

                            (Type::Array(t1), Type::Array(t2)) => {
                                if **t1 == Type::Any {
                                    Ok(Type::Array(t2.clone()))
                                } else if **t2 == Type::Any {
                                    Ok(Type::Array(t1.clone()))
                                } else if t1 == t2 {
                                    Ok(Type::Array(t1.clone()))
                                } else {

                                    Ok(Type::Array(Box::new(Type::Any)))
                                }
                            }

                            (Type::Any, _) | (_, Type::Any) => Ok(Type::Any),
                            _ => Err(format!("Type Error at {}: Cannot add types {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::Sub | BinaryOperator::Mul | BinaryOperator::Div | BinaryOperator::Mod => {
                         match (&left_type, &right_type) {
                            (Type::Int, Type::Int) => Ok(Type::Int),
                            (Type::Float, Type::Float) => Ok(Type::Float),
                            (Type::Int, Type::Float) => Ok(Type::Float),
                            (Type::Float, Type::Int) => Ok(Type::Float),

                            (Type::Any, _) | (_, Type::Any) => Ok(Type::Any),
                            _ => Err(format!("Type Error at {}: Cannot perform arithmetic on types {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::Power => {
                        match (&left_type, &right_type) {
                            (Type::Int, Type::Int) => Ok(Type::Int),
                            (Type::Float, Type::Float) => Ok(Type::Float),
                            (Type::Int, Type::Float) => Ok(Type::Float),
                            (Type::Float, Type::Int) => Ok(Type::Float),

                            (Type::Any, _) | (_, Type::Any) => Ok(Type::Any),
                            _ => Err(format!("Type Error at {}: Power operator (^) requires numeric types, got {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual => Ok(Type::Bool),
                    BinaryOperator::Less | BinaryOperator::Greater | BinaryOperator::LessEqual | BinaryOperator::GreaterEqual => {
                        match (&left_type, &right_type) {
                            (Type::Int, Type::Int) | (Type::Float, Type::Float) => Ok(Type::Bool),

                            (Type::Any, _) | (_, Type::Any) => Ok(Type::Bool),
                            _ => Err(format!("Type Error at {}: Cannot perform ordered comparison on types {:?} and {:?}", loc, left_type, right_type)),
                        }
                    }
                    BinaryOperator::And | BinaryOperator::Or => {
                        match (&left_type, &right_type) {
                            (Type::Bool, Type::Bool) => Ok(Type::Bool),

                            (Type::Any, _) | (_, Type::Any) => Ok(Type::Bool),
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
                            return Err(format!(
                                "Type Error: Unary operator '!' cannot be applied to type {:?}",
                                operand_type
                            ));
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

            ASTNode::QuantumDeclaration {
                name,
                size,
                initial_state,
            } => {
                let register_type: Type;
                if let Some(size_expr) = size {
                    let size_type = Self::check(size_expr, env, Option::None)?;
                    if size_type != Type::Int {
                        return Err(format!(
                            "Type Error: Quantum register size must be an Int, but got {:?}",
                            size_type
                        ));
                    }
                    let size_val = if let ASTNode::IntLiteral(n) = &**size_expr {
                        Some(*n as usize)
                    } else {
                        None
                    };
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
                env.borrow_mut()
                    .set(name.clone(), Self::immutable_info(register_type));
                Ok(Type::None)
            }

            ASTNode::ArrayAccess { array, index, loc } => {
                let array_type = Self::check(array, env, Option::None)?;
                let index_type = Self::check(index, env, Option::None)?;

                match array_type {
                    Type::QuantumRegister(size_opt) => {
                        if index_type != Type::Int {
                            return Err(format!(
                            "Type Error at {}: Quantum register index must be an Int, but got {:?}",
                            loc, index_type
                        ));
                        }
                        if let (Some(size), ASTNode::IntLiteral(idx)) = (size_opt, &**index) {
                            if *idx < 0 || *idx as usize >= size {
                                return Err(format!(
                                "Type Error at {}: Qubit index {} is out of bounds for register of size {}.",
                                loc, idx, size
                            ));
                            }
                        }
                        Ok(Type::Qubit)
                    }

                    Type::Array(inner_type) => {
                        if index_type != Type::Int {
                            return Err(format!(
                                "Type Error at {}: Array index must be an Int, but got {:?}",
                                loc, index_type
                            ));
                        }
                        Ok(*inner_type)
                    }

                    Type::String => {
                        if index_type != Type::Int {
                            return Err(format!(
                                "Type Error at {}: String index must be an Int, but got {:?}",
                                loc, index_type
                            ));
                        }
                        Ok(Type::String)
                    }

                    Type::Dict => {
                        if index_type != Type::String
                            && index_type != Type::Int
                            && index_type != Type::Bool
                        {
                            return Err(format!(
                            "Type Error at {}: Dictionary key must be String, Int, or Bool, but got {:?}",
                            loc, index_type
                        ));
                        }
                        Ok(Type::Any)
                    }

                    Type::Custom(ref name) if name == "Dict" => {
                        if index_type != Type::String
                            && index_type != Type::Int
                            && index_type != Type::Bool
                        {
                            return Err(format!(
                            "Type Error at {}: Dictionary key must be String, Int, or Bool, but got {:?}",
                            loc, index_type
                        ));
                        }
                        Ok(Type::Any)
                    }

                    _ => Err(format!(
                        "Type Error at {}: Cannot perform array access '[]' on type {:?}",
                        loc, array_type
                    )),
                }
            }

            ASTNode::MemberAccess { object, member } => {
                let object_type = Self::check(object, env, Option::None)?;

                let class_name_opt = match &object_type {
                    Type::Instance(name) | Type::Custom(name) => Some(name.clone()),
                    _ => None,
                };

                if let Some(mut class_name) = class_name_opt {
                    if class_name.contains('.') {
                        if let Some(real_name) = class_name.split('.').last() {
                            class_name = real_name.to_string();
                        }
                    }

                    let field_key = format!("{}::{}", class_name, member);

                    if let Some(field_info) = env.borrow().get(&field_key) {
                        return Ok(field_info.var_type.clone());
                    }

                    let mut found_type = None;
                    let mut current_env = Some(env.clone());

                    while let Some(env_rc) = current_env {
                        let env_ref = env_rc.borrow();
                        for info in env_ref.store.values() {
                            if let Type::Module(mod_types) = &info.var_type {
                                if let Some(field_type) = mod_types.get(&field_key) {
                                    found_type = Some(field_type.clone());
                                    break;
                                }
                            }
                        }
                        if found_type.is_some() { break; }
                        current_env = env_ref.outer.clone();
                    }

                    if let Some(t) = found_type {
                        return Ok(t);
                    }

                    return Err(format!(
                        "Type Error: Class '{}' has no member named '{}'",
                        class_name, member
                    ));
                }


                if let Type::Module(module_types) = object_type {
                    match module_types.get(member) {
                        Some(t) => Ok(t.clone()),
                        None => Err(format!(
                            "Type Error: Module has no member named '{}'",
                            member
                        )),
                    }

                } else if member == "length" {
                    match object_type {
                        Type::Array(_) | Type::String | Type::Dict => Ok(Type::Int),
                        Type::QuantumRegister(_) => Ok(Type::Int),
                        _ => Err(format!(
                            "Type Error: Cannot get .length of type {:?}",
                            object_type
                        )),
                    }
                } else {
                    if let Type::Dict = object_type {
                        Ok(Type::Any)
                    } else {
                        Err(format!(
                            "Type Error: Type {:?} has no member named '{}'",
                            object_type, member
                        ))
                    }
                }
            }
            ASTNode::Dagger { .. } => Self::check_gate_expression(node, env),

            ASTNode::Apply {
                gate_expr,
                arguments,
                loc,
            } => {
                let gate_type = Self::check_gate_expression(gate_expr, env)?;

                match gate_type {
                    Type::Function(param_types, return_type) => {
                        if arguments.len() != param_types.len() {
                            return Err(format!(
                                "Type Error at {}: Gate requires {} arguments, but got {}",
                                loc,
                                param_types.len(),
                                arguments.len()
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
                    _ => Err(format!(
                        "Type Error at {}: This expression is not a callable gate.",
                        loc
                    )),
                }
            }

            ASTNode::Gate { .. } => Self::check_gate_expression(node, env),

            ASTNode::Controlled { gate_expr, loc } => Self::check_gate_expression(node, env),

            ASTNode::Measure(target_expr) => {
                let target_type = Self::check(target_expr, env, Option::None)?;
                match target_type {
                    Type::Qubit | Type::Any => Ok(Type::Int),
                    _ => Err(format!(
                        "Type Error: 'measure' can only be used on a single Qubit, got {:?}",
                        target_type
                    )),
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
                                env.borrow_mut()
                                    .set(name.clone(), Self::immutable_info(var_type.clone()));
                            } else {
                                return Err(format!("Type Error: Cannot import name '{}'. It does not exist in module.", name));
                            }
                        }
                    }
                }
                Ok(Type::None)
            }

            ASTNode::FunctionDeclaration {
                name,
                parameters,
                return_type,
                body,
                ..
            } => {
                let param_types: Vec<Type> =
                    parameters.iter().map(|p| p.param_type.clone()).collect();
                let rt = return_type.clone().unwrap_or(Type::Any);
                let func_type = Type::Function(param_types, Box::new(rt.clone()));
                env.borrow_mut()
                    .set(name.clone(), Self::immutable_info(func_type));
                let func_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));
                for param in parameters {
                    func_env.borrow_mut().set(
                        param.name.clone(),
                        Self::immutable_info(param.param_type.clone()),
                    );
                }
                Self::check(body, &func_env, Some(&rt))?;
                Ok(Type::None)
            }

            ASTNode::CircuitDeclaration {
                name,
                parameters,
                return_type,
                body,
                ..
            } => {
                let param_types: Vec<Type> =
                    parameters.iter().map(|p| p.param_type.clone()).collect();
                let rt = return_type.clone().unwrap_or(Type::Any);
                let func_type = Type::Function(param_types, Box::new(rt.clone()));
                env.borrow_mut()
                    .set(name.clone(), Self::immutable_info(func_type));
                let func_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));
                for param in parameters {
                    func_env.borrow_mut().set(
                        param.name.clone(),
                        Self::immutable_info(param.param_type.clone()),
                    );
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
                    Option::None => Err(
                        "Type Error: 'return' statement found outside of a function.".to_string(),
                    ),
                    Some(expected) => {
                        if !Self::is_type_compatible(&value_type, expected) {
                            return Err(format!(
                                "Type Error: Function expected return type {:?}, but found return with type {:?}",
                                expected, value_type
                            ));
                        }
                        Ok(Type::None)
                    }
                }
            }

            ASTNode::While { condition, body } => {
                let cond_type = Self::check(condition, env, Option::None)?;
                if cond_type != Type::Bool {
                    return Err(format!(
                        "Type Error: 'while' loop condition must be a Bool, but got {:?}",
                        cond_type
                    ));
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
                    return Err(format!(
                        "Type Error: Range start must be an Int, got {:?}",
                        start_type
                    ));
                }
                if end_type != Type::Int {
                    return Err(format!(
                        "Type Error: Range end must be an Int, got {:?}",
                        end_type
                    ));
                }

                Ok(Type::Custom("range".to_string()))
            }

            ASTNode::For {
                variable,
                iterator,
                body,
            } => {
                let iterator_type = Self::check(iterator, env, Option::None)?;
                let element_type = match iterator_type {
                    Type::Array(inner_type) => *inner_type,
                    Type::String => Type::String,
                    Type::Dict => Type::String,
                    Type::Custom(name) if name == "range" => Type::Int,
                    Type::Any => Type::Any,
                    _ => {
                        return Err(format!(
                            "Type Error: 'for' loop cannot iterate over type {:?}",
                            iterator_type
                        ))
                    }
                };
                let loop_env = Rc::new(RefCell::new(TypeEnvironment::new_enclosed(env.clone())));
                loop_env
                    .borrow_mut()
                    .set(variable.clone(), Self::immutable_info(element_type));
                Self::check(body, &loop_env, expected_return_type)?;
                Ok(Type::None)
            }

            ASTNode::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                let cond_type = Self::check(condition, env, Option::None)?;
                if cond_type != Type::Bool {
                    return Err(format!(
                        "Type Error: 'if' condition must be a Bool, but got {:?}",
                        cond_type
                    ));
                }
                let then_type = Self::check(then_block, env, expected_return_type)?;
                for (elif_cond, elif_body) in elif_blocks {
                    let elif_cond_type = Self::check(elif_cond, env, Option::None)?;
                    if elif_cond_type != Type::Bool {
                        return Err(format!(
                            "Type Error: 'elif' condition must be a Bool, but got {:?}",
                            elif_cond_type
                        ));
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

            ASTNode::TryCatch {
                try_block,
                catch_block,
                ..
            } => {
                Self::check(try_block, env, expected_return_type)?;
                Self::check(catch_block, env, expected_return_type)?;
                Ok(Type::None)
            }
            ASTNode::Break => Ok(Type::None),
            ASTNode::Continue => Ok(Type::None),

            _ => Err(format!(
                "Type checking is not implemented for this node: {:?}",
                node
            )),
        }
    }

    fn is_type_compatible(actual: &Type, expected: &Type) -> bool {
        if actual == expected {
            return true;
        }

        match (actual, expected) {
            (Type::Any, _) | (_, Type::Any) => true,

            (Type::None, _) => true,

            (Type::QuantumRegister(_), Type::QuantumRegister(None)) => true,

            (Type::Instance(actual_class), Type::Custom(expected_class)) => {
                if actual_class == expected_class {
                    return true;
                }
                if expected_class.contains('.') {
                    if let Some(class_name) = expected_class.split('.').last() {
                        return actual_class == class_name;
                    }
                }
                false
            }

            (Type::Custom(actual_class), Type::Instance(expected_class)) => {
                if actual_class == expected_class {
                    return true;
                }
                if actual_class.contains('.') {
                    if let Some(class_name) = actual_class.split('.').last() {
                        return class_name == expected_class;
                    }
                }
                false
            }

            (Type::Custom(actual_class), Type::Custom(expected_class)) => {
                actual_class == expected_class ||
                actual_class.ends_with(&format!(".{}", expected_class)) ||
                expected_class.ends_with(&format!(".{}", actual_class))
            }

            (Type::Dict, Type::Custom(name)) | (Type::Custom(name), Type::Dict)
                if name == "Dict" => true,

            (Type::Array(t1), Type::Array(t2)) => {
                **t1 == Type::Any || **t2 == Type::Any || Self::is_type_compatible(t1, t2)
            }

            (Type::Int, Type::Float) => true,

            _ => false,
        }
    }
}
