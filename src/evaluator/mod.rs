/* src/evaluator/mod.rs */
use crate::environment::{Environment, GateDefinition, RuntimeValue};
use crate::lexer::Lexer;
use crate::parser::ast::ASTNode;
use crate::parser::ast::BinaryOperator;
use crate::parser::ast::ClassField;
use crate::parser::ast::ClassMethod;
use crate::parser::ast::ImportPath;
use crate::parser::ast::ImportSpec;
use crate::parser::ast::Loc;
use crate::parser::Parser;
use num_complex::Complex;
use rand::Rng;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
type C64 = Complex<f64>;
use crate::graphics::{Canvas, Color, Plot, PlotType};
use std::io::Write;
use std::sync::Mutex;
type QubitMap = HashMap<String, usize>;
lazy_static::lazy_static! {
    static ref INTERPRETER_CANVASES: Mutex<HashMap<i64, Canvas>> = Mutex::new(HashMap::new());
    static ref INTERPRETER_PLOTS: Mutex<HashMap<i64, Plot>> = Mutex::new(HashMap::new());
    static ref NEXT_ID: Mutex<i64> = Mutex::new(0);
}
use crate::quantum_backend::native_simulator::NativeSimulator;
use crate::qubit_lifecycle::{QubitId, QubitLifecycleManager, QubitOperation, QubitState};

pub struct Evaluator;

impl Evaluator {
    pub fn evaluate_program(
        program: &ASTNode,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        if let ASTNode::Program(statements) = program {
            let mut last_result = RuntimeValue::None;
            for stmt in statements {
                last_result = Self::evaluate(stmt, env)?;
            }
            Ok(last_result)
        } else {
            Err("Expected ASTNode::Program at root.".to_string())
        }
    }

    pub fn evaluate(
        node: &ASTNode,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        match node {
            ASTNode::LetDeclaration { name, value, .. } => Self::eval_let_declaration(name, value, env),
            ASTNode::Assignment { target, value } => Self::eval_assignment(target, value, env),
            ASTNode::IntLiteral(n) => Ok(RuntimeValue::Int(*n)),
            ASTNode::FloatLiteral(f) => Ok(RuntimeValue::Float(*f)),
            ASTNode::StringLiteral(s) => Ok(RuntimeValue::String(s.clone())),
            ASTNode::BoolLiteral(b) => Ok(RuntimeValue::Bool(*b)),
            ASTNode::NoneLiteral => Ok(RuntimeValue::None),
            ASTNode::Identifier { name, loc } => Self::eval_identifier(name, loc, env),
            ASTNode::MemberAccess { object, member, .. } => Self::eval_member_access(object, member, env),
            ASTNode::FunctionCall { callee, arguments, loc, is_dagger } =>
                Self::eval_function_call(callee, arguments, loc, env, *is_dagger),
            ASTNode::Binary { operator, left, right, loc } => Self::eval_binary_op(operator, left, right, loc, env),
            ASTNode::Unary { operator, operand, .. } => Self::eval_unary_op(operator, operand, env),
            ASTNode::Break => Ok(RuntimeValue::Break),
            ASTNode::Continue => Ok(RuntimeValue::Continue),
            ASTNode::TryCatch { try_block, error_variable, catch_block } => {
                Self::eval_try_catch(try_block, error_variable, catch_block, env)
            }
            ASTNode::Import { path, alias } => {
                Self::eval_import_statement(path, alias, env)
            }
            ASTNode::FromImport { path, spec } => {
                 Self::eval_from_import(path, spec, env)
            }
            ASTNode::ClassDeclaration { name, superclass, fields, methods, constructor, .. } => {
                Self::eval_class_declaration(name, superclass, fields, methods, constructor, env)
            }

            ASTNode::NewInstance { class_name, arguments, loc } => {
                Self::eval_new_instance(class_name, arguments, loc, env)
            }

            ASTNode::MethodCall { object, method_name, arguments, loc } => {
                Self::eval_method_call(object, method_name, arguments, loc, env)
            }

            ASTNode::SelfRef { loc } => {
                Self::eval_self_ref(loc, env)
            }
            ASTNode::If { condition, then_block, elif_blocks, else_block } =>
                Self::eval_if_statement(condition, then_block, elif_blocks, else_block, env),
            ASTNode::While { condition, body } =>
                Self::eval_while_statement(condition, body, env),
            ASTNode::For { variable, iterator, body } =>
                Self::eval_for_statement(variable, iterator, body, env),
            ASTNode::Block(statements) =>
                Self::eval_block(statements, env),
            ASTNode::Range { start, end, inclusive } =>
                Self::eval_range_expression(start, end, *inclusive, env),

            ASTNode::QuantumDeclaration { name, size, initial_state } =>
                Self::eval_quantum_declaration(name, size, initial_state, env),
            ASTNode::QuantumKet(s) => {
                let size = 1;
                let state_vec = Self::ket_to_state_vector(s, size)?;
                Ok(RuntimeValue::QuantumRegister {
                    size,
                    state: Rc::new(RefCell::new(state_vec)),
                    register_name: "ket".to_string(),
                    global_start_index: None,
                })
            },
            ASTNode::Apply { gate_expr, arguments, loc } => {
                Self::eval_apply_statement(gate_expr, arguments, loc, env)
            }
            ASTNode::Gate { .. } | ASTNode::Controlled { .. } | ASTNode::Dagger { .. } => {
                Err("Gate expressions (like 'X' or 'controlled(X)') can only be used inside an 'apply' statement.".to_string())
            }

            ASTNode::Measure(target_expr) =>
                Self::eval_measure(target_expr, env),
            ASTNode::ArrayAccess { array, index, loc } =>
                Self::eval_array_access(array, index, loc, env),
            ASTNode::ArrayLiteral(element_exprs) => {
                let mut elements = Vec::new();
                for expr in element_exprs {
                    let val = Self::evaluate(expr, env)?;
                    elements.push(std::rc::Rc::new(std::cell::RefCell::new(val)));
                }
                Ok(RuntimeValue::Register(elements))
            }
            ASTNode::DictLiteral(pairs) => {
                Self::eval_dict_literal(pairs, env)
            }
            ASTNode::FunctionDeclaration { name, parameters, return_type:_, body, .. } => {
                let func = RuntimeValue::Function {
                    parameters: parameters.clone(),
                    body: body.clone(),
                    env: env.clone(),
                };
                env.borrow_mut().set(name.clone(), func);
                Ok(RuntimeValue::None)
            }
            ASTNode::CircuitDeclaration { name, parameters, return_type:_, body, .. } => {
                let func = RuntimeValue::Function {
                    parameters: parameters.clone(),
                    body: body.clone(),
                    env: env.clone(),
                };
                env.borrow_mut().set(name.clone(), func);
                Ok(RuntimeValue::None)
            }
            ASTNode::Return(value_expr) => {
                let value = match value_expr {
                    Some(expr) => Self::evaluate(expr, env)?,
                    None => RuntimeValue::None,
                };
                Ok(RuntimeValue::ReturnValue(Box::new(value)))
            }
            _ => Err(format!("Evaluation not implemented for AST node: {:?}", node)),
        }
    }

    pub fn register_builtins(env: &Rc<RefCell<Environment>>) {
        let mut e = env.borrow_mut();

        e.set("time".to_string(), RuntimeValue::BuiltinFunction("time".to_string()));
        e.set("matrix_update".to_string(), RuntimeValue::BuiltinFunction("matrix_update".to_string()));
        e.set("compute_input_gradient".to_string(), RuntimeValue::BuiltinFunction("compute_input_gradient".to_string()));
        e.set(
            "file_append".to_string(),
            RuntimeValue::BuiltinFunction("file_append".to_string()),
        );

        e.set(
            "print".to_string(),
            RuntimeValue::BuiltinFunction("print".to_string()),
        );
        e.set(
            "len".to_string(),
            RuntimeValue::BuiltinFunction("len".to_string()),
        );
        e.set(
            "to_string".to_string(),
            RuntimeValue::BuiltinFunction("to_string".to_string()),
        );
        e.set(
            "to_int".to_string(),
            RuntimeValue::BuiltinFunction("to_int".to_string()),
        );
        e.set(
            "to_float".to_string(),
            RuntimeValue::BuiltinFunction("to_float".to_string()),
        );
        e.set(
            "type_of".to_string(),
            RuntimeValue::BuiltinFunction("type_of".to_string()),
        );
        e.set(
            "assert".to_string(),
            RuntimeValue::BuiltinFunction("assert".to_string()),
        );
        e.set(
            "echo".to_string(),
            RuntimeValue::BuiltinFunction("echo".to_string()),
        );
        e.set(
            "maybe".to_string(),
            RuntimeValue::BuiltinFunction("maybe".to_string()),
        );
        e.set(
            "sample".to_string(),
            RuntimeValue::BuiltinFunction("sample".to_string()),
        );
        e.set(
            "debug_state".to_string(),
            RuntimeValue::BuiltinFunction("debug_state".to_string()),
        );
        e.set(
            "measure".to_string(),
            RuntimeValue::BuiltinFunction("measure".to_string()),
        );

        e.set(
            "_graphics_create_canvas".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_create_canvas".to_string()),
        );
        e.set(
            "_graphics_set_background".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_set_background".to_string()),
        );
        e.set(
            "_graphics_draw_rect".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_draw_rect".to_string()),
        );
        e.set(
            "_graphics_draw_circle".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_draw_circle".to_string()),
        );
        e.set(
            "_graphics_draw_line".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_draw_line".to_string()),
        );
        e.set(
            "_graphics_draw_text".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_draw_text".to_string()),
        );
        e.set(
            "_graphics_save_svg".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_save_svg".to_string()),
        );
        e.set(
            "_graphics_destroy_canvas".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_destroy_canvas".to_string()),
        );

        e.set(
            "_graphics_create_plot".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_create_plot".to_string()),
        );
        e.set(
            "_graphics_plot_set_data".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_plot_set_data".to_string()),
        );
        e.set(
            "_graphics_plot_set_title".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_plot_set_title".to_string()),
        );
        e.set(
            "_graphics_plot_render".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_plot_render".to_string()),
        );
        e.set(
            "_graphics_destroy_plot".to_string(),
            RuntimeValue::BuiltinFunction("_graphics_destroy_plot".to_string()),
        );

        e.set(
            "file_write".to_string(),
            RuntimeValue::BuiltinFunction("file_write".to_string()),
        );
        e.set(
            "file_read".to_string(),
            RuntimeValue::BuiltinFunction("file_read".to_string()),
        );
    }
    fn is_truthy(val: &RuntimeValue) -> bool {
        match val {
            RuntimeValue::Bool(b) => *b,
            RuntimeValue::Int(n) => *n != 0,
            RuntimeValue::None => false,
            RuntimeValue::Probabilistic { value, confidence } => {
                if rand::thread_rng().gen::<f64>() < *confidence {
                    Self::is_truthy(value)
                } else {
                    false
                }
            }
            _ => true,
        }
    }
    fn eval_class_declaration(
        name: &str,
        superclass: &Option<String>,
        fields: &Vec<ClassField>,
        methods: &Vec<ClassMethod>,
        constructor: &Option<Box<ASTNode>>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let mut method_map = HashMap::new();
        for method in methods {
            method_map.insert(method.name.clone(), method.clone());
        }

        let class_value = RuntimeValue::Class {
            name: name.to_string(),
            superclass: superclass.clone(),
            fields: fields.clone(),
            methods: method_map,
            constructor: constructor.clone(),
            definition_env: Some(env.clone()),
        };

        env.borrow_mut().set(name.to_string(), class_value);
        Ok(RuntimeValue::None)
    }

    fn eval_new_instance(
        class_name: &str,
        arguments: &Vec<ASTNode>,
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let parts: Vec<&str> = class_name.split('.').collect();

        let class_value_rc = if parts.len() > 1 {
            let module_name = parts[0];
            let target_class = parts[1];

            let module_rc = env.borrow().get(module_name).ok_or_else(|| {
                format!("Runtime Error at {}: Unknown module '{}'", loc, module_name)
            })?;

            let module_val = module_rc.borrow();
            if let RuntimeValue::Module(mod_env) = &*module_val {
                mod_env.borrow().get(target_class).ok_or_else(|| {
                    format!(
                        "Runtime Error at {}: Module '{}' has no class '{}'",
                        loc, module_name, target_class
                    )
                })?
            } else {
                return Err(format!(
                    "Runtime Error at {}: '{}' is not a module.",
                    loc, module_name
                ));
            }
        } else {
            env.borrow().get(class_name).ok_or_else(|| {
                format!("Runtime Error at {}: Unknown class '{}'", loc, class_name)
            })?
        };

        let class_borrow = class_value_rc.borrow();
        let (fields_def, methods, constructor, def_env) = match &*class_borrow {
            RuntimeValue::Class {
                fields,
                methods,
                constructor,
                definition_env,
                ..
            } => (
                fields.clone(),
                methods.clone(),
                constructor.clone(),
                definition_env.clone(),
            ),
            _ => {
                return Err(format!(
                    "Runtime Error at {}: '{}' is not a class",
                    loc, class_name
                ))
            }
        };
        drop(class_borrow);

        let mut instance_fields = HashMap::new();
        let mut initial_values = HashMap::new();
        for field in &fields_def {
            let value = if let Some(default_expr) = &field.default_value {
                Self::evaluate(default_expr, env)?
            } else {
                RuntimeValue::None
            };

            let shared_ptr = Rc::new(RefCell::new(value));
            initial_values.insert(field.name.clone(), shared_ptr.clone());
            instance_fields.insert(field.name.clone(), shared_ptr);
        }

        let instance = RuntimeValue::Instance {
            class_name: class_name.to_string(),
            fields: instance_fields.clone(),
            methods: methods.clone(),
            definition_env: def_env.clone(),
        };

        if let Some(constructor_node) = constructor {
            if let ASTNode::FunctionDeclaration {
                parameters, body, ..
            } = &*constructor_node
            {
                let mut evaluated_args = Vec::new();
                for arg in arguments {
                    evaluated_args.push(Self::evaluate(arg, env)?);
                }

                if parameters.len() != evaluated_args.len() {
                    return Err(format!("Runtime Error: Constructor arg mismatch."));
                }

                let parent_env = def_env.unwrap_or(env.clone());
                let constructor_env = Rc::new(RefCell::new(Environment::new_enclosed(parent_env)));

                constructor_env
                    .borrow_mut()
                    .set("self".to_string(), instance.clone());

                for (name, val_rc) in &initial_values {
                    constructor_env
                        .borrow_mut()
                        .define_rc(name.clone(), val_rc.clone());
                }

                for (param, arg_value) in parameters.iter().zip(evaluated_args) {
                    constructor_env
                        .borrow_mut()
                        .set(param.name.clone(), arg_value);
                }

                Self::evaluate(&body, &constructor_env)?;
            }
        }

        Ok(instance)
    }
    fn are_values_equal(a: &RuntimeValue, b: &RuntimeValue) -> bool {
        match (a, b) {
            (RuntimeValue::None, RuntimeValue::None) => true,
            (RuntimeValue::Int(x), RuntimeValue::Int(y)) => x == y,
            (RuntimeValue::Float(x), RuntimeValue::Float(y)) => (x - y).abs() < f64::EPSILON,
            (RuntimeValue::Bool(x), RuntimeValue::Bool(y)) => x == y,
            (RuntimeValue::String(x), RuntimeValue::String(y)) => x == y,

            _ => false,
        }
    }

    fn eval_method_call(
        object_expr: &ASTNode,
        method_name: &str,
        arguments: &Vec<ASTNode>,
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let object = Self::evaluate(object_expr, env)?;

        match object {
            RuntimeValue::Instance {
                class_name,
                fields,
                methods,
                definition_env,
            } => {
                let method = methods.get(method_name).ok_or_else(|| {
                    format!(
                        "Runtime Error at {}: Class '{}' has no method '{}'",
                        loc, class_name, method_name
                    )
                })?;

                let mut evaluated_args = Vec::new();
                for arg in arguments {
                    evaluated_args.push(Self::evaluate(arg, env)?);
                }

                if method.parameters.len() != evaluated_args.len() {
                    return Err(format!(
                        "Runtime Error at {}: Method '{}' expected {} arguments, got {}",
                        loc,
                        method_name,
                        method.parameters.len(),
                        evaluated_args.len()
                    ));
                }

                let parent_env = definition_env.unwrap_or(env.clone());
                let method_env = Rc::new(RefCell::new(Environment::new_enclosed(parent_env)));

                let self_instance = RuntimeValue::Instance {
                    class_name: class_name.clone(),
                    fields: fields.clone(),
                    methods: methods.clone(),
                    definition_env: None,
                };
                method_env
                    .borrow_mut()
                    .set("self".to_string(), self_instance);

                for (field_name, field_value_rc) in &fields {
                    method_env
                        .borrow_mut()
                        .define_rc(field_name.clone(), field_value_rc.clone());
                }

                for (param, arg_value) in method.parameters.iter().zip(evaluated_args) {
                    method_env.borrow_mut().set(param.name.clone(), arg_value);
                }

                let result = Self::evaluate(&method.body, &method_env)?;

                for (field_name, field_value_rc) in &fields {
                    method_env
                        .borrow_mut()
                        .define_rc(field_name.clone(), field_value_rc.clone());
                }
                match result {
                    RuntimeValue::ReturnValue(val) => Ok(*val),
                    other => Ok(other),
                }
            }

            RuntimeValue::Module(module_env) => {
                let member_val = module_env.borrow().get(method_name).ok_or_else(|| {
                    format!(
                        "Runtime Error at {}: Module does not have a member named '{}'",
                        loc, method_name
                    )
                })?;

                let member_val = member_val.borrow().clone();
                let evaluated_args = Self::eval_arguments(arguments, env)?;

                match member_val {

                    RuntimeValue::Function {
                        parameters,
                        body,
                        env: func_env,
                    } => {
                        let mut function_scope = Environment::new_enclosed(func_env);
                        if parameters.len() != evaluated_args.len() {
                            return Err(format!(
                                "Runtime Error at {}: Module function '{}.{}' expected {} arguments, but got {}.",
                                loc, "module", method_name, parameters.len(), evaluated_args.len()
                            ));
                        }
                        for (param, arg_val) in parameters.iter().zip(evaluated_args) {
                            function_scope.set(param.name.clone(), arg_val);
                        }
                        let function_scope_rc = Rc::new(RefCell::new(function_scope));

                        let statements = match &*body {
                            ASTNode::Block(stmts) => stmts,
                            _ => return Err(format!("Internal Error: Function body is not a Block.")),
                        };

                        let result = Self::eval_block(statements, &function_scope_rc)?;

                        if let RuntimeValue::ReturnValue(val) = result {
                            Ok(*val)
                        } else {
                            Ok(result)
                        }
                    }


                    RuntimeValue::Class { name, fields, methods, constructor, .. } => {

                        let mut instance_fields = HashMap::new();
                        let mut initial_values = HashMap::new();

                        for field in &fields {
                            let value = if let Some(default_expr) = &field.default_value {
                                Self::evaluate(default_expr, env)?
                            } else {
                                RuntimeValue::None
                            };

                            let shared_ptr = Rc::new(RefCell::new(value));
                            initial_values.insert(field.name.clone(), shared_ptr.clone());
                            instance_fields.insert(field.name.clone(), shared_ptr);
                        }


                        let mut instance = RuntimeValue::Instance {
                            class_name: name.clone(),
                            fields: instance_fields.clone(),
                            methods: methods.clone(),
                            definition_env: None,
                        };


                        if let Some(constructor_node) = constructor {
                            if let ASTNode::FunctionDeclaration { parameters, body, .. } = &*constructor_node {
                                if parameters.len() != evaluated_args.len() {
                                    return Err(format!("Runtime Error: Constructor arg mismatch."));
                                }

                                let constructor_env = Rc::new(RefCell::new(Environment::new_enclosed(module_env.clone())));


                                constructor_env.borrow_mut().set("self".to_string(), instance.clone());


                                for (name, val) in &initial_values {
                                    constructor_env.borrow_mut().define_rc(name.clone(), val.clone());
                                }


                                for (param, arg_value) in parameters.iter().zip(evaluated_args) {
                                    constructor_env.borrow_mut().set(param.name.clone(), arg_value);
                                }


                                Self::evaluate(&body, &constructor_env)?;

                                /*
                                for (field_name, field_rc) in &instance_fields {
                                    if let Some(local_val_rc) = constructor_env.borrow().get(field_name) {
                                        let local_val = local_val_rc.borrow().clone();

                                        if let Some(initial_val_rc) = initial_values.get(field_name) {
                                            let initial_val = initial_val_rc.borrow();

                                            if !Self::are_values_equal(&local_val, &*initial_val) {
                                                *field_rc.borrow_mut() = local_val;
                                            }
                                        }
                                    }
                                }
                                */

                                instance = RuntimeValue::Instance {
                                    class_name: name.clone(),
                                    fields: instance_fields,
                                    methods,
                                    definition_env: None,
                                };
                            }
                        }
                        Ok(instance)
                    }

                    RuntimeValue::BuiltinFunction(func_name) => {
                         Err(format!(
                            "Runtime Error at {}: Built-in function '{}' cannot be called via module access yet.",
                            loc, func_name
                        ))
                    }
                    _ => Err(format!(
                        "Runtime Error at {}: '{}.{}' is not a function or class.",
                        loc, "module", method_name
                    )),
                }
            }

            _ => Err(format!(
                "Runtime Error at {}: Cannot call method '{}' on type {}",
                loc,
                method_name,
                object.type_name()
            )),
        }
    }

    fn eval_self_ref(loc: &Loc, env: &Rc<RefCell<Environment>>) -> Result<RuntimeValue, String> {
        Err(format!(
            "Runtime Error at {}: 'self' can only be used inside class methods",
            loc
        ))
    }

    fn eval_quantum_declaration(
        name: &str,
        size_expr: &Option<Box<ASTNode>>,
        initial_state_expr: &Option<Box<ASTNode>>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let size = if let Some(expr) = size_expr {
            let size_val = Self::evaluate(expr, env)?;
            match size_val {
                RuntimeValue::Int(n) if n > 0 => n as usize,
                _ => return Err(format!("Runtime Error: Size must be > 0 int.")),
            }
        } else {
            1
        };

        if size > 25 {
            return Err(format!("Runtime Error: Size {} too large.", size));
        }

        let state_map = Self::default_state_vector(size)?;
        let register = RuntimeValue::QuantumRegister {
            size,
            state: Rc::new(RefCell::new(state_map)),
            register_name: name.to_string(),
            global_start_index: None,
        };

        env.borrow_mut().set(name.to_string(), register.clone());
        Ok(register)
    }
    fn eval_unary_op(
        operator: &crate::parser::ast::UnaryOperator,
        operand_expr: &Box<ASTNode>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let operand = Self::evaluate(operand_expr, env)?;
        match operator {
            crate::parser::ast::UnaryOperator::Not => {
                Ok(RuntimeValue::Bool(!Self::is_truthy(&operand)))
            }
            crate::parser::ast::UnaryOperator::Minus => match operand {
                RuntimeValue::Int(i) => Ok(RuntimeValue::Int(-i)),
                RuntimeValue::Float(f) => Ok(RuntimeValue::Float(-f)),
                _ => Err(format!(
                    "Runtime Error: Unary operator '-' not defined for type {:?}",
                    operand.type_name()
                )),
            },
            crate::parser::ast::UnaryOperator::Plus => match operand {
                RuntimeValue::Int(i) => Ok(RuntimeValue::Int(i)),
                RuntimeValue::Float(f) => Ok(RuntimeValue::Float(f)),
                _ => Err(format!(
                    "Runtime Error: Unary operator '+' not defined for type {:?}",
                    operand.type_name()
                )),
            },
        }
    }

    fn default_state_vector(size: usize) -> Result<HashMap<usize, (f64, f64)>, String> {
        if size == 0 {
            return Err("Runtime Error: Quantum register size must be > 0".to_string());
        }

        let mut state = HashMap::new();
        state.insert(0, (1.0, 0.0));
        Ok(state)
    }

    fn ket_to_state_vector(
        ket_str: &str,
        size: usize,
    ) -> Result<HashMap<usize, (f64, f64)>, String> {
        if size != 1 {
            return Err(
                "Runtime Error: Can only initialize single-qubit kets for now.".to_string(),
            );
        }
        let mut state = HashMap::new();
        match ket_str {
            "0" => state.insert(0, (1.0, 0.0)),
            "1" => state.insert(1, (1.0, 0.0)),
            _ => {
                return Err(format!(
                    "Runtime Error: Invalid single-qubit ket state '{}'.",
                    ket_str
                ))
            }
        };
        Ok(state)
    }

    fn complex_mul((a, b): (f64, f64), (c, d): (f64, f64)) -> (f64, f64) {
        (a * c - b * d, a * d + b * c)
    }
    fn complex_add((a, b): (f64, f64), (c, d): (f64, f64)) -> (f64, f64) {
        (a + c, b + d)
    }
    fn complex_scalar_mul((a, b): (f64, f64), s: f64) -> (f64, f64) {
        (a * s, b * s)
    }
    fn complex_mul_by_phase((a, b): (f64, f64), phi: f64) -> (f64, f64) {
        let (c, d) = (phi.cos(), phi.sin());
        (a * c - b * d, a * d + b * c)
    }
    fn complex_norm_sq((a, b): (f64, f64)) -> f64 {
        a * a + b * b
    }
    fn eval_apply_statement(
        gate_expr_node: &ASTNode,
        arg_nodes: &[ASTNode],
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let mut qubit_args = Vec::new();
        for arg_node in arg_nodes {
            qubit_args.push(Self::evaluate(arg_node, env)?);
        }

        let gate_val = Self::eval_gate_expression(gate_expr_node, env)?;

        let (base_name, is_dagger, num_controls) = match gate_val {
            RuntimeValue::Gate {
                base_name,
                is_dagger,
                num_controls,
            } => (base_name, is_dagger, num_controls),
            _ => {
                return Err(format!(
                    "Runtime Error at {}: Expression is not a valid gate.",
                    loc
                ))
            }
        };

        let mut controls = Vec::new();
        let mut targets = Vec::new();

        let mut state_rc: Option<Rc<RefCell<HashMap<usize, (f64, f64)>>>> = None;
        let mut reg_size: Option<usize> = None;

        if qubit_args.len() < num_controls {
            return Err(format!("Runtime Error at {}: Gate requires {} control qubits, but only {} arguments provided.", loc, num_controls, qubit_args.len()));
        }

        let (_control_args, _target_args) = qubit_args.split_at(num_controls);

        for (i, qubit_val) in qubit_args.iter().enumerate() {
            let (q_state_rc, q_index, q_size) = match qubit_val {
                RuntimeValue::Qubit {
                    state, index, size, ..
                } => (state.clone(), *index, *size),
                _ => {
                    return Err(format!(
                    "Runtime Error at {}: Gate arguments must be Qubits, but argument {} was {}.",
                    loc,
                    i + 1,
                    qubit_val.type_name()
                ))
                }
            };

            if i == 0 {
                state_rc = Some(q_state_rc);
                reg_size = Some(q_size);
            } else {
                if !Rc::ptr_eq(state_rc.as_ref().unwrap(), &q_state_rc) {
                    return Err(format!("Runtime Error at {}: All qubits in an 'apply' statement must be from the same register.", loc));
                }
            }

            if i < num_controls {
                controls.push(q_index);
            } else {
                targets.push(q_index);
            }
        }

        let mut params = Vec::new();
        Self::extract_gate_params(gate_expr_node, &mut params, env)?;

        let gate_def = GateDefinition {
            name: base_name,
            params,
            controls,
            targets,
            register_size: reg_size.unwrap_or(0),
            state_rc: state_rc.unwrap(),
        };

        Self::apply_multi_controlled_gate(gate_def, is_dagger)
    }

    fn eval_gate_expression(
        node: &ASTNode,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        match node {
            ASTNode::Gate { name, loc: _ } => Ok(RuntimeValue::Gate {
                base_name: name.to_lowercase(),
                is_dagger: false,
                num_controls: 0,
            }),

            ASTNode::ParameterizedGate { name, .. } => Ok(RuntimeValue::Gate {
                base_name: name.to_lowercase(),
                is_dagger: false,
                num_controls: 0,
            }),
            ASTNode::Dagger { gate_expr, .. } => {
                let inner_gate = Self::eval_gate_expression(gate_expr, env)?;
                match inner_gate {
                    RuntimeValue::Gate {
                        base_name,
                        is_dagger,
                        num_controls,
                    } => Ok(RuntimeValue::Gate {
                        base_name,
                        is_dagger: !is_dagger,
                        num_controls,
                    }),
                    _ => Err("Internal Error: 'dagger' did not receive a valid gate.".to_string()),
                }
            }
            ASTNode::Controlled { gate_expr, .. } => {
                let inner_gate = Self::eval_gate_expression(gate_expr, env)?;
                match inner_gate {
                    RuntimeValue::Gate {
                        base_name,
                        is_dagger,
                        num_controls,
                    } => Ok(RuntimeValue::Gate {
                        base_name,
                        is_dagger,
                        num_controls: num_controls + 1,
                    }),
                    _ => {
                        Err("Internal Error: 'controlled' did not receive a valid gate."
                            .to_string())
                    }
                }
            }
            _ => Err("Internal Error: Invalid ASTNode passed to eval_gate_expression.".to_string()),
        }
    }

    fn extract_gate_info_runtime(gate_expr: &ASTNode) -> Result<(String, bool), String> {
        match gate_expr {
            ASTNode::Gate { name, .. } => Ok((name.to_lowercase(), false)),
            ASTNode::ParameterizedGate { name, .. } => Ok((name.to_lowercase(), false)),

            ASTNode::Dagger { gate_expr, .. } => {
                let (name, is_dagger) = Self::extract_gate_info_runtime(gate_expr)?;
                Ok((name, !is_dagger))
            }

            ASTNode::Controlled { gate_expr, .. } => {
                let (name, is_dagger) = Self::extract_gate_info_runtime(gate_expr)?;
                Ok((format!("c{}", name), is_dagger))
            }

            _ => Err("Invalid gate expression".to_string()),
        }
    }

    fn extract_gate_params(
        node: &ASTNode,
        params: &mut Vec<f64>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<(), String> {
        match node {
            ASTNode::Gate { name, .. } => match name.to_lowercase().as_str() {
                "rx" | "ry" | "rz" | "cphase" | "u" => {
                    return Err(format!("Runtime Error: Parameterized gate '{}' must be called as a function (e.g., RX(theta)).", name));
                }
                _ => Ok(()),
            },
            ASTNode::ParameterizedGate {
                parameters: param_exprs,
                ..
            } => {
                for param_expr in param_exprs {
                    let param_val = Self::evaluate(param_expr, env)?;
                    let float_param = match param_val {
                        RuntimeValue::Float(f) => f,
                        RuntimeValue::Int(i) => i as f64,
                        _ => {
                            return Err(format!(
                                "Runtime Error: Gate parameter must be a number, got {:?}",
                                param_val.type_name()
                            ))
                        }
                    };
                    params.push(float_param);
                }
                Ok(())
            }
            ASTNode::Dagger { gate_expr, .. } => Self::extract_gate_params(gate_expr, params, env),
            ASTNode::Controlled { gate_expr, .. } => {
                Self::extract_gate_params(gate_expr, params, env)
            }

            ASTNode::FunctionCall {
                callee,
                arguments,
                loc,
                ..
            } => {
                let gate_name = match &**callee {
                    ASTNode::Identifier { name, .. } => name.to_lowercase(),
                    _ => return Err(format!("Runtime Error at {}: Gate expression must be a simple identifier inside the call.", loc)),
                };

                for arg_node in arguments.iter() {
                    let arg_val = Self::evaluate(arg_node, env)?;
                    let float_param = match arg_val {
                        RuntimeValue::Float(f) => f,
                        RuntimeValue::Int(i) => i as f64,
                        _ => {
                            break;
                        }
                    };
                    params.push(float_param);
                }
                Ok(())
            }

            _ => Ok(()),
        }
    }

    pub fn apply_multi_controlled_gate(
        gate: GateDefinition,
        is_dagger: bool,
    ) -> Result<RuntimeValue, String> {
        let is_native_2qubit = matches!(
            gate.name.as_str(),
            "cphase" | "cnot" | "cz" | "cx" | "cy" | "swap"
        );

        if is_native_2qubit && gate.controls.is_empty() && gate.targets.len() == 2 {
            return Self::apply_native_2qubit_gate(gate, is_dagger);
        }

        let matrix = Self::get_gate_matrix(&gate.name, &gate.params, is_dagger)?;

        let mut control_mask = 0;
        for &control_idx in &gate.controls {
            if (control_mask & (1 << control_idx)) != 0 {
                return Err("Runtime Error: Duplicate control qubit indices.".to_string());
            }
            for &target_idx in &gate.targets {
                if control_idx == target_idx {
                    return Err(
                        "Runtime Error: Control and target qubits must be different.".to_string(),
                    );
                }
            }
            control_mask |= 1 << control_idx;
        }

        if gate.targets.is_empty() {
            return Err(format!(
                "Runtime Error: Gate '{}' must have at least 1 target qubit.",
                gate.name
            ));
        }
        if gate.targets.len() > 1 {
            return Err(
                "Runtime Error: Multi-target gates with additional controls are not yet supported."
                    .to_string(),
            );
        }

        let target_idx = gate.targets[0];
        let target_mask = 1 << target_idx;

        let mut state_map_guard = gate.state_rc.borrow_mut();
        let old_state_map = &*state_map_guard;
        let mut new_state_map = HashMap::new();

        let mut processed = std::collections::HashSet::new();

        for (&basis_state, &amp_tuple) in old_state_map.iter() {
            if processed.contains(&basis_state) {
                continue;
            }

            if (basis_state & control_mask) == control_mask {
                let partner_state = basis_state ^ target_mask;

                let amp_self_tuple = amp_tuple;
                let amp_partner_tuple = old_state_map
                    .get(&partner_state)
                    .cloned()
                    .unwrap_or((0.0, 0.0));

                let amp_self = C64::new(amp_self_tuple.0, amp_self_tuple.1);
                let amp_partner = C64::new(amp_partner_tuple.0, amp_partner_tuple.1);

                let (amp0, amp1, idx0, idx1);

                if (basis_state & target_mask) == 0 {
                    amp0 = amp_self;
                    amp1 = amp_partner;
                    idx0 = basis_state;
                    idx1 = partner_state;
                } else {
                    amp0 = amp_partner;
                    amp1 = amp_self;
                    idx0 = partner_state;
                    idx1 = basis_state;
                }

                let new_amp0 = matrix[0][0] * amp0 + matrix[0][1] * amp1;
                let new_amp1 = matrix[1][0] * amp0 + matrix[1][1] * amp1;

                Self::insert_if_nonzero(&mut new_state_map, idx0, new_amp0);
                Self::insert_if_nonzero(&mut new_state_map, idx1, new_amp1);

                processed.insert(idx0);
                processed.insert(idx1);
            } else {
                new_state_map.insert(basis_state, amp_tuple);
                processed.insert(basis_state);
            }
        }

        *state_map_guard = new_state_map;
        Ok(RuntimeValue::None)
    }

    fn apply_native_2qubit_gate(
        gate: GateDefinition,
        is_dagger: bool,
    ) -> Result<RuntimeValue, String> {
        if gate.targets.len() != 2 {
            return Err(format!(
                "Runtime Error: Gate '{}' requires exactly 2 qubits.",
                gate.name
            ));
        }

        let control_idx = gate.targets[0];
        let target_idx = gate.targets[1];

        if control_idx == target_idx {
            return Err("Runtime Error: Control and target qubits must be different.".to_string());
        }

        let control_mask = 1 << control_idx;
        let target_mask = 1 << target_idx;

        let mut state_map_guard = gate.state_rc.borrow_mut();

        match gate.name.as_str() {
            "cphase" => {
                let phi = gate.params.get(0).cloned().unwrap_or(0.0);
                let angle = if is_dagger { -phi } else { phi };
                let phase_factor = C64::from_polar(1.0, angle);

                for (&basis_state, amp_tuple) in state_map_guard.iter_mut() {
                    if (basis_state & control_mask) != 0 && (basis_state & target_mask) != 0 {
                        let amp = C64::new(amp_tuple.0, amp_tuple.1);
                        let new_amp = amp * phase_factor;
                        *amp_tuple = (new_amp.re, new_amp.im);
                    }
                }
            }
            "swap" => {
                let mut new_state_map = HashMap::new();
                for (&basis_state, &amp_tuple) in state_map_guard.iter() {
                    let bit_a = (basis_state & control_mask) != 0;
                    let bit_b = (basis_state & target_mask) != 0;

                    if bit_a != bit_b {
                        let swapped_idx = basis_state ^ control_mask ^ target_mask;
                        new_state_map.insert(swapped_idx, amp_tuple);
                    } else {
                        new_state_map.insert(basis_state, amp_tuple);
                    }
                }
                *state_map_guard = new_state_map;
            }
            "cnot" | "cx" => {
                let mut new_state_map = HashMap::new();
                for (&basis_state, &amp_tuple) in state_map_guard.iter() {
                    if (basis_state & control_mask) != 0 {
                        let swapped_idx = basis_state ^ target_mask;
                        new_state_map.insert(swapped_idx, amp_tuple);
                    } else {
                        new_state_map.insert(basis_state, amp_tuple);
                    }
                }
                *state_map_guard = new_state_map;
            }
            "cz" => {
                for (&basis_state, amp_tuple) in state_map_guard.iter_mut() {
                    if (basis_state & control_mask) != 0 && (basis_state & target_mask) != 0 {
                        *amp_tuple = (-amp_tuple.0, -amp_tuple.1);
                    }
                }
            }
            _ => {
                return Err(format!(
                    "Runtime Error: Native 2-qubit gate '{}' not implemented.",
                    gate.name
                ));
            }
        }

        Ok(RuntimeValue::None)
    }

    fn get_gate_matrix(
        name: &str,
        params: &[f64],
        is_dagger: bool,
    ) -> Result<[[C64; 2]; 2], String> {
        let i = C64::new(0.0, 1.0);
        let eff_dagger = if is_dagger { -1.0 } else { 1.0 };

        match name {
            "x" | "not" | "cnot" | "ccx" | "toffoli" => Ok([
                [C64::new(0.0, 0.0), C64::new(1.0, 0.0)],
                [C64::new(1.0, 0.0), C64::new(0.0, 0.0)],
            ]),
            "y" => Ok([[C64::new(0.0, 0.0), -i], [i, C64::new(0.0, 0.0)]]),
            "z" | "cz" => Ok([
                [C64::new(1.0, 0.0), C64::new(0.0, 0.0)],
                [C64::new(0.0, 0.0), C64::new(-1.0, 0.0)],
            ]),
            "h" | "hadamard" => {
                let v = 1.0 / std::f64::consts::SQRT_2;
                Ok([
                    [C64::new(v, 0.0), C64::new(v, 0.0)],
                    [C64::new(v, 0.0), C64::new(-v, 0.0)],
                ])
            }
            "s" | "cs" => {
                let angle = std::f64::consts::FRAC_PI_2 * eff_dagger;
                Ok([
                    [C64::new(1.0, 0.0), C64::new(0.0, 0.0)],
                    [C64::new(0.0, 0.0), C64::from_polar(1.0, angle)],
                ])
            }
            "t" | "ct" => {
                let angle = std::f64::consts::FRAC_PI_4 * eff_dagger;
                Ok([
                    [C64::new(1.0, 0.0), C64::new(0.0, 0.0)],
                    [C64::new(0.0, 0.0), C64::from_polar(1.0, angle)],
                ])
            }
            "rx" => {
                let theta = params.get(0).cloned().unwrap_or(0.0) * eff_dagger;
                let t_2 = theta / 2.0;
                let (c, s) = (t_2.cos(), t_2.sin());
                Ok([
                    [C64::new(c, 0.0), C64::new(0.0, -s)],
                    [C64::new(0.0, -s), C64::new(c, 0.0)],
                ])
            }
            "ry" => {
                let theta = params.get(0).cloned().unwrap_or(0.0) * eff_dagger;
                let t_2 = theta / 2.0;
                let (c, s) = (t_2.cos(), t_2.sin());
                Ok([
                    [C64::new(c, 0.0), C64::new(-s, 0.0)],
                    [C64::new(s, 0.0), C64::new(c, 0.0)],
                ])
            }
            "rz" => {
                let theta = params.get(0).cloned().unwrap_or(0.0) * eff_dagger;
                let t_2 = theta / 2.0;
                Ok([
                    [C64::from_polar(1.0, -t_2), C64::new(0.0, 0.0)],
                    [C64::new(0.0, 0.0), C64::from_polar(1.0, t_2)],
                ])
            }
            "cphase" => {
                let phi = params.get(0).cloned().unwrap_or(0.0) * eff_dagger;
                Ok([
                    [C64::new(1.0, 0.0), C64::new(0.0, 0.0)],
                    [C64::new(0.0, 0.0), C64::from_polar(1.0, phi)],
                ])
            }
            "u" => {
                if params.len() < 3 {
                    return Err("U gate requires theta, phi, lambda.".to_string());
                }
                let (t, p, l) = (
                    params[0] * eff_dagger,
                    params[1] * eff_dagger,
                    params[2] * eff_dagger,
                );
                let t_2 = t / 2.0;

                let c00 = C64::new(t_2.cos(), 0.0);
                let c01 = C64::from_polar(-t_2.sin(), l);
                let c10 = C64::from_polar(t_2.sin(), p);
                let c11 = C64::from_polar(t_2.cos(), p + l);

                Ok([[c00, c01], [c10, c11]])
            }

            _ => Err(format!(
                "Runtime Error: Unknown gate name '{}' for matrix generation.",
                name
            )),
        }
    }

    fn eval_measure(
        target_expr: &Box<ASTNode>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let target_val = Self::evaluate(target_expr, env)?;
        let (state_rc, target_index, reg_size) = match target_val {
            RuntimeValue::Qubit {
                state, index, size, ..
            } => (state, index, size),
            _ => {
                return Err(format!(
                    "Runtime Error: 'measure' can only be used on a single Qubit, got {}.",
                    target_val.type_name()
                ))
            }
        };

        Self::perform_measurement(&state_rc, target_index, reg_size)
    }

    fn perform_measurement(
        state_rc: &Rc<RefCell<HashMap<usize, (f64, f64)>>>,
        target_index: usize,
        total_size: usize,
    ) -> Result<RuntimeValue, String> {
        let mut state_map_guard = state_rc.borrow_mut();
        let old_state_map = &*state_map_guard;

        if target_index >= total_size {
            return Err(format!(
                "Runtime Error: Qubit index {} is out of bounds for size {}.",
                target_index, total_size
            ));
        }

        let target_mask = 1 << target_index;
        let mut prob0 = 0.0;

        for (&basis_state, &amp_tuple) in old_state_map.iter() {
            if (basis_state & target_mask) == 0 {
                prob0 += amp_tuple.0.powi(2) + amp_tuple.1.powi(2);
            }
        }

        let mut rng = rand::thread_rng();
        let rand_val: f64 = rng.gen_range(0.0..1.0);
        let measured_result: i64;
        let probability_of_outcome: f64;

        if rand_val < prob0 {
            measured_result = 0;
            probability_of_outcome = prob0;
        } else {
            measured_result = 1;
            probability_of_outcome = 1.0 - prob0;
        }

        let norm_factor = if probability_of_outcome.abs() < 1e-9 {
            1.0
        } else {
            1.0 / probability_of_outcome.sqrt()
        };

        let mut new_state_map = HashMap::new();

        for (&basis_state, &amp_tuple) in old_state_map.iter() {
            let bit_at_index = (basis_state >> target_index) & 1;

            if bit_at_index as i64 == measured_result {
                let (real, imag) = amp_tuple;
                let normalized_amp = Self::complex_scalar_mul((real, imag), norm_factor);
                new_state_map.insert(basis_state, normalized_amp);
            }
        }

        *state_map_guard = new_state_map;

        Ok(RuntimeValue::Int(measured_result))
    }

    fn eval_array_access(
        collection_expr: &Box<ASTNode>,
        index_expr: &Box<ASTNode>,
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let collection_val = Self::evaluate(collection_expr, env)?;
        let index_val = Self::evaluate(index_expr, env)?;

        match collection_val {
            RuntimeValue::Register(elements) => {
                let index = match index_val {
                    RuntimeValue::Int(i) => i as usize,
                    _ => {
                        return Err(format!(
                            "Runtime Error at {}: Array index must be an integer.",
                            loc
                        ))
                    }
                };
                elements
                    .get(index)
                    .map(|e| e.borrow().clone())
                    .ok_or(format!(
                        "Runtime Error at {}: Array index {} out of bounds for array of size {}.",
                        loc,
                        index,
                        elements.len()
                    ))
            }

            RuntimeValue::QuantumRegister {
                size,
                state,
                register_name,
                global_start_index,
            } => {
                let index = match index_val {
                    RuntimeValue::Int(i) => i,
                    _ => return Err(format!("Runtime Error at {}: Index must be int.", loc)),
                };
                if index < 0 || index as usize >= size {
                    return Err(format!("Runtime Error at {}: Index out of bounds.", loc));
                }

                let global_index = global_start_index.map(|start| start + (index as usize));

                Ok(RuntimeValue::Qubit {
                    state: state.clone(),
                    index: index as usize,
                    size,
                    register_name: register_name.clone(),
                    global_index,
                })
            }

            RuntimeValue::Dict(map) => {
                let key = Self::value_to_string_key(index_val)?;
                map.get(&key).map(|v| v.borrow().clone()).ok_or(format!(
                    "Runtime Error at {}: Key '{}' not found in dictionary.",
                    loc, key
                ))
            }
            _ => Err(format!(
                "Runtime Error at {}: Subscript '[]' is not supported for type {:?}",
                loc,
                collection_val.type_name()
            )),
        }
    }

    fn eval_member_access(
        object_expr: &ASTNode,
        member: &str,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let object = Self::evaluate(object_expr, env)?;
        match object {
            RuntimeValue::Module(module_env) => match module_env.borrow().get(member) {
                Some(value_rc) => Ok(value_rc.borrow().clone()),
                None => Err(format!(
                    "Runtime Error: Module does not have a member named '{}'",
                    member
                )),
            },
            RuntimeValue::Instance { fields, .. } => match fields.get(member) {
                Some(value_rc) => Ok(value_rc.borrow().clone()),
                None => Err(format!(
                    "Runtime Error: Instance has no field named '{}'",
                    member
                )),
            },

            RuntimeValue::QuantumRegister { size, .. } => match member {
                "length" => Ok(RuntimeValue::Int(size as i64)),
                _ => Err(format!(
                    "Runtime Error: QuantumRegister does not have a member named '{}'",
                    member
                )),
            },
            RuntimeValue::Register(rc_register) => match member {
                "length" => Ok(RuntimeValue::Int(rc_register.len() as i64)),
                _ => Err(format!(
                    "Runtime Error: Array does not have a member named '{}'",
                    member
                )),
            },
            RuntimeValue::String(s) => match member {
                "length" => Ok(RuntimeValue::Int(s.len() as i64)),
                _ => Err(format!(
                    "Runtime Error: String does not have a member named '{}'",
                    member
                )),
            },
            RuntimeValue::Dict(map) => map.get(member).map(|v| v.borrow().clone()).ok_or(format!(
                "Runtime Error: Value does not have a member named '{}'",
                member
            )),
            _ => Err(format!(
                "Runtime Error: Member access (.) is not supported for type {:?}",
                object.type_name()
            )),
        }
    }

    fn insert_if_nonzero(state_map: &mut HashMap<usize, (f64, f64)>, index: usize, amp: C64) {
        if amp.norm_sqr() > 1e-12 {
            state_map.insert(index, (amp.re, amp.im));
        }
    }

    fn eval_range_expression(
        start_expr: &Box<ASTNode>,
        end_expr: &Box<ASTNode>,
        inclusive: bool,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let start_val = Self::evaluate(start_expr, env)?;
        let end_val = Self::evaluate(end_expr, env)?;
        match (start_val, end_val) {
            (RuntimeValue::Int(start), RuntimeValue::Int(end)) => {
                let range: Vec<i64> = if inclusive {
                    (start..=end).collect()
                } else {
                    (start..end).collect()
                };
                Ok(RuntimeValue::Range(range))
            }
            _ => Err("Runtime Error: Range boundaries must be integers.".to_string()),
        }
    }

    fn eval_from_import(
        path: &ImportPath,
        spec: &ImportSpec,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let file_path = Self::resolve_import_path(path)?;
        let source = fs::read_to_string(&file_path).map_err(|e| {
            format!(
                "Runtime Error: Failed to import file '{}': {}",
                file_path, e
            )
        })?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Import Lexer Error: {}", e))?;
        let mut parser = Parser::new(tokens);
        let ast = parser
            .parse()
            .map_err(|e| format!("Import Parser Error: {}", e))?;
        let module_env = Rc::new(RefCell::new(Environment::new()));
        Self::evaluate_program(&ast, &module_env)?;
        let module_store = module_env.borrow().get_store_clone();
        match spec {
            ImportSpec::List(names) => {
                for name in names {
                    match module_store.get(name) {
                        Some(value_rc) => env
                            .borrow_mut()
                            .set(name.clone(), value_rc.borrow().clone()),
                        None => {
                            return Err(format!(
                                "Runtime Error: Cannot import name '{}' from file '{}'.",
                                name, file_path
                            ))
                        }
                    }
                }
            }
            ImportSpec::All => {
                for (name, value_rc) in module_store.iter() {
                    env.borrow_mut()
                        .set(name.clone(), value_rc.borrow().clone());
                }
            }
        }
        Ok(RuntimeValue::None)
    }

    fn eval_try_catch(
        try_block: &ASTNode,
        error_variable: &Option<String>,
        catch_block: &ASTNode,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        match Self::evaluate(try_block, env) {
            Ok(value) => Ok(value),
            Err(error_message) => {
                let mut catch_env = Environment::new_enclosed(env.clone());
                if let Some(var_name) = error_variable {
                    catch_env.set(var_name.clone(), RuntimeValue::String(error_message));
                }
                let catch_env_rc = Rc::new(RefCell::new(catch_env));
                Self::evaluate(catch_block, &catch_env_rc)
            }
        }
    }

    fn eval_for_statement(
        variable_name: &str,
        iterator_expr: &Box<ASTNode>,
        body: &Box<ASTNode>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let iterator_val = Self::evaluate(iterator_expr, env)?;
        let iterable: Vec<RuntimeValue> = match iterator_val {
            RuntimeValue::Range(range_vec) => {
                range_vec.into_iter().map(RuntimeValue::Int).collect()
            }
            RuntimeValue::Register(elements) => elements
                .iter()
                .map(|rc_cell| rc_cell.borrow().clone())
                .collect(),
            _ => {
                return Err(format!(
                    "Runtime Error: For loop iterator must be a range or an array, not {:?}.",
                    iterator_val.type_name()
                ))
            }
        };
        for item in iterable {
            env.borrow_mut().set(variable_name.to_string(), item);
            let body_result = Self::evaluate(body, env)?;
            match body_result {
                RuntimeValue::Break => break,
                RuntimeValue::Continue => continue,
                RuntimeValue::ReturnValue(val) => return Ok(RuntimeValue::ReturnValue(val)),
                _ => {}
            }
        }
        Ok(RuntimeValue::None)
    }

    fn eval_function_call(
        callee_expr: &ASTNode,
        arguments: &[ASTNode],
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
        is_dagger: bool,
    ) -> Result<RuntimeValue, String> {
        let evaluated_args = Self::eval_arguments(arguments, env)?;
        let function = Self::evaluate(callee_expr, env)?;
        let name = format!("{:?}", callee_expr);

        match function {
            RuntimeValue::BuiltinFunction(func_name) => {
                if is_dagger {
                    return Err(format!(
                        "Runtime Error at {}: Dagger is not supported for built-in function '{}'.",
                        loc, func_name
                    ));
                }

                match func_name.as_str() {
                    "print" => Self::builtin_print(evaluated_args),
                    "matrix_update" => Self::builtin_matrix_update(evaluated_args),
                    "compute_input_gradient" => Self::builtin_compute_input_gradient(evaluated_args),
                    "time" => Self::builtin_time(evaluated_args),
                    "input" => Self::builtin_input(evaluated_args),
                    "split" => Self::builtin_split(evaluated_args),
                    "echo" => Self::builtin_echo(evaluated_args),
                    "maybe" => Self::builtin_maybe(evaluated_args),
                    "sample" => Self::builtin_sample(evaluated_args),
                    "type_of" => Self::builtin_type_of(evaluated_args),
                    "to_string" => Self::builtin_to_string(evaluated_args),
                    "to_int" => Self::builtin_to_int(evaluated_args),
                    "to_float" => Self::builtin_to_float(evaluated_args),
                    "len" => Self::builtin_len(evaluated_args),
                    "debug_state" => Self::builtin_debug_state(evaluated_args),
                    "assert" => Self::builtin_assert(evaluated_args),
                    "_graphics_create_canvas" => {
                        Self::builtin_graphics_create_canvas(evaluated_args)
                    }
                    "_graphics_set_background" => {
                        Self::builtin_graphics_set_background(evaluated_args)
                    }
                    "_graphics_draw_rect" => Self::builtin_graphics_draw_rect(evaluated_args),
                    "_graphics_draw_circle" => Self::builtin_graphics_draw_circle(evaluated_args),
                    "_graphics_draw_line" => Self::builtin_graphics_draw_line(evaluated_args),
                    "_graphics_save_svg" => Self::builtin_graphics_save_svg(evaluated_args),
                    "_graphics_destroy_canvas" => Ok(RuntimeValue::None),
                    "_graphics_create_plot" => Self::builtin_graphics_create_plot(evaluated_args),
                    "_graphics_plot_set_data" => {
                        Self::builtin_graphics_plot_set_data(evaluated_args)
                    }
                    "_graphics_plot_set_title" => {
                        Self::builtin_graphics_plot_set_title(evaluated_args)
                    }
                    "_graphics_plot_render" => Self::builtin_graphics_plot_render(evaluated_args),
                    "_graphics_destroy_plot" => Self::builtin_graphics_destroy_plot(evaluated_args),

                    "file_write" => Self::builtin_file_write(evaluated_args),
                    "file_read" => Self::builtin_file_read(evaluated_args),
                    "file_append" => Self::builtin_file_append(evaluated_args),

                    _ => Err(format!(
                        "Runtime Error at {}: Unknown built-in function '{}'.",
                        loc, func_name
                    )),
                }
            }
            RuntimeValue::Function {
                parameters,
                body,
                env: func_env,
            } => {
                let mut function_scope = Environment::new_enclosed(func_env);
                if parameters.len() != evaluated_args.len() {
                    return Err(format!(
                        "Runtime Error at {}: Function '{}' expected {} arguments, but got {}.",
                        loc,
                        name,
                        parameters.len(),
                        evaluated_args.len()
                    ));
                }
                for (param, arg_val) in parameters.iter().zip(evaluated_args) {
                    function_scope.set(param.name.clone(), arg_val);
                }
                let function_scope_rc = Rc::new(RefCell::new(function_scope));

                let statements = match &*body {
                    ASTNode::Block(stmts) => stmts,
                    _ => {
                        return Err(format!(
                            "Internal Error: Function body for '{}' is not a Block.",
                            name
                        ))
                    }
                };

                let result = if is_dagger {
                    Self::eval_daggered_block(statements, &function_scope_rc)?
                } else {
                    Self::eval_block(statements, &function_scope_rc)?
                };

                if let RuntimeValue::ReturnValue(val) = result {
                    Ok(*val)
                } else {
                    Ok(result)
                }
            }

            RuntimeValue::Class {
                name,
                fields,
                methods,
                constructor,
                ..
            } => {
                let mut instance_fields = HashMap::new();
                let mut initial_values = HashMap::new();
                for field in &fields {
                    let value = if let Some(default_expr) = &field.default_value {
                        Self::evaluate(default_expr, env)?
                    } else {
                        RuntimeValue::None
                    };
                    initial_values.insert(field.name.clone(), value.clone());
                    instance_fields.insert(field.name.clone(), Rc::new(RefCell::new(value)));
                }

                let mut instance = RuntimeValue::Instance {
                    class_name: name.clone(),
                    fields: instance_fields.clone(),
                    methods: methods.clone(),
                    definition_env: None,
                };

                if let Some(constructor_node) = constructor {
                    if let ASTNode::FunctionDeclaration {
                        parameters, body, ..
                    } = &*constructor_node
                    {
                        if parameters.len() != evaluated_args.len() {
                            return Err(format!("Runtime Error: Constructor arg mismatch."));
                        }

                        let constructor_env =
                            Rc::new(RefCell::new(Environment::new_enclosed(env.clone())));

                        constructor_env
                            .borrow_mut()
                            .set("self".to_string(), instance.clone());

                        for (name, val) in &initial_values {
                            constructor_env.borrow_mut().set(name.clone(), val.clone());
                        }

                        for (param, arg_value) in parameters.iter().zip(evaluated_args) {
                            constructor_env
                                .borrow_mut()
                                .set(param.name.clone(), arg_value);
                        }

                        Self::evaluate(&body, &constructor_env)?;

                        for (field_name, field_rc) in &instance_fields {
                            if let Some(local_val_rc) = constructor_env.borrow().get(field_name) {
                                let local_val = local_val_rc.borrow().clone();

                                if let Some(initial_val) = initial_values.get(field_name) {
                                    if !Self::are_values_equal(&local_val, initial_val) {
                                        *field_rc.borrow_mut() = local_val;
                                    }
                                }
                            }
                        }

                        instance = RuntimeValue::Instance {
                            class_name: name.clone(),
                            fields: instance_fields,
                            methods,
                            definition_env: None,
                        };
                    }
                }
                Ok(instance)
            }
            _ => Err(format!(
                "Runtime Error at {}: '{}' is not a callable function.",
                loc, name
            )),
        }
    }

    fn eval_dict_literal(
        pairs: &Vec<(ASTNode, ASTNode)>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let mut map = HashMap::new();
        for (key_node, value_node) in pairs {
            let key_str = match key_node {
                ASTNode::Identifier { name, .. } => name.clone(),
                ASTNode::StringLiteral(s) => s.clone(),
                _ => {
                    let key_val = Self::evaluate(key_node, env)?;
                    Self::value_to_string_key(key_val)?
                }
            };
            let value_val = Self::evaluate(value_node, env)?;
            map.insert(key_str, Rc::new(RefCell::new(value_val)));
        }
        Ok(RuntimeValue::Dict(map))
    }

    fn resolve_import_path(path: &ImportPath) -> Result<String, String> {
        match path {
            ImportPath::File(file_path) => Ok(file_path.clone()),
            ImportPath::Module(segments) => {
                let mut pbuf = PathBuf::new();
                for segment in segments {
                    pbuf.push(segment);
                }
                pbuf.set_extension("qc");
                pbuf.to_str()
                    .map(|s| s.to_string())
                    .ok_or("Runtime Error: Invalid non-UTF8 module path.".to_string())
            }
        }
    }

    fn eval_daggered_block(
        statements: &Vec<ASTNode>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let mut last_result = RuntimeValue::None;

        for stmt in statements.iter().rev() {
            let flipped_node = match stmt {
                ASTNode::Apply {
                    gate_expr,
                    arguments,
                    loc,
                } => {
                    let daggered_gate_expr = match &**gate_expr {
                        ASTNode::Gate { name, loc, .. } => ASTNode::Dagger {
                            gate_expr: Box::new(ASTNode::Gate {
                                name: name.clone(),
                                loc: *loc,
                            }),
                            loc: *loc,
                        },

                        ASTNode::Dagger { gate_expr, .. } => *gate_expr.clone(),

                        ASTNode::Controlled { gate_expr, loc } => {
                            let inner_gate = match &**gate_expr {
                            ASTNode::Gate { name, loc, .. } => {
                                ASTNode::Dagger {
                                    gate_expr: Box::new(ASTNode::Gate { name: name.clone(), loc: *loc }),
                                    loc: *loc
                                }
                            }
                            ASTNode::Dagger { gate_expr, .. } => {
                                *gate_expr.clone()
                            }
                            ASTNode::ParameterizedGate { name, parameters, loc } => {
                                ASTNode::Dagger {
                                    gate_expr: Box::new(ASTNode::ParameterizedGate {
                                        name: name.clone(),
                                        parameters: parameters.clone(),
                                        loc: *loc
                                    }),
                                    loc: *loc
                                }
                            }
                            _ => return Err("Daggering nested complex gate expressions is not yet supported.".to_string())
                        };
                            ASTNode::Controlled {
                                gate_expr: Box::new(inner_gate),
                                loc: *loc,
                            }
                        }

                        ASTNode::ParameterizedGate {
                            name,
                            parameters,
                            loc,
                        } => ASTNode::Dagger {
                            gate_expr: Box::new(ASTNode::ParameterizedGate {
                                name: name.clone(),
                                parameters: parameters.clone(),
                                loc: *loc,
                            }),
                            loc: *loc,
                        },
                        _ => {
                            return Err("Daggering complex gate expressions is not yet supported."
                                .to_string())
                        }
                    };

                    ASTNode::Apply {
                        gate_expr: Box::new(daggered_gate_expr),
                        arguments: arguments.clone(),
                        loc: *loc,
                    }
                }
                ASTNode::FunctionCall {
                    callee,
                    arguments,
                    loc,
                    is_dagger: original_is_dagger,
                } => ASTNode::FunctionCall {
                    callee: callee.clone(),
                    arguments: arguments.clone(),
                    loc: *loc,
                    is_dagger: !*original_is_dagger,
                },
                _ => stmt.clone(),
            };

            last_result = Self::evaluate(&flipped_node, env)?;

            match last_result {
                RuntimeValue::ReturnValue(_) | RuntimeValue::Break | RuntimeValue::Continue => {
                    return Ok(last_result);
                }
                _ => {}
            }
        }
        Ok(last_result)
    }

    pub fn print_quantum_state(
        state: &Rc<RefCell<HashMap<usize, (f64, f64)>>>,
        size: usize,
        max_entries: usize,
    ) {
        let num_qubits = size;
        let amplitudes_map = state.borrow();
        println!(
            "--- Quantum State ({} qubits, {} non-zero amplitudes) ---",
            num_qubits,
            amplitudes_map.len()
        );

        let mut sorted_keys: Vec<usize> = amplitudes_map.keys().cloned().collect();
        sorted_keys.sort();

        for (count, i) in sorted_keys.iter().enumerate() {
            if count >= max_entries {
                println!("  ... ({} more entries hidden)", sorted_keys.len() - count);
                break;
            }
            let (real, imag) = amplitudes_map[i];
            let basis_state = format!("{:0width$b}", i, width = num_qubits);
            println!("  |{}> : {:.6} + {:.6}i", basis_state, real, imag);
        }
        println!("-----------------------------------");
    }

    fn builtin_file_append(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 2 {
            return Err(
                "Runtime Error: 'file_append' expects 2 arguments (path, content).".to_string(),
            );
        }

        let path = match &args[0] {
            RuntimeValue::String(s) => s,
            _ => return Err("Runtime Error: File path must be a String.".to_string()),
        };

        let content = match &args[1] {
            RuntimeValue::String(s) => s,
            _ => return Err("Runtime Error: File content must be a String.".to_string()),
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("File Append Error: {}", e))?;

        write!(file, "{}", content).map_err(|e| format!("File Write Error: {}", e))?;
        Ok(RuntimeValue::None)
    }

    fn builtin_compute_input_gradient(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 2 && args.len() != 4 {
            return Err("Runtime Error: 'compute_input_gradient' requires 2 or 4 args".to_string());
        }
        let weights_matrix = match &args[0] {
            RuntimeValue::Register(v) => v,
            _ => return Err("Runtime Error: Weights must be a matrix".to_string())
        };

        let delta_vec = match &args[1] {
            RuntimeValue::Register(v) => v,
            _ => return Err("Runtime Error: Delta must be an array".to_string())
        };

        let output_size = delta_vec.len();
        if output_size == 0 { return Ok(RuntimeValue::Register(vec![])); }

        if weights_matrix.is_empty() { return Ok(RuntimeValue::Register(vec![])); }
        let input_size = match &*weights_matrix[0].borrow() {
            RuntimeValue::Register(row) => row.len(),
            _ => 0
        };

        let mut input_gradient = vec![0.0; input_size];

        //result[j] = sum(weights[i][j] * delta[i])
        for (i, row_rc) in weights_matrix.iter().enumerate() {
            if i >= output_size { break; }

            // delta[i]
            let d_val = match *delta_vec[i].borrow() {
                RuntimeValue::Float(f) => f,
                _ => 0.0
            };

            let row_val = row_rc.borrow();
            if let RuntimeValue::Register(row) = &*row_val {
                for (j, weight_rc) in row.iter().enumerate() {
                    if j >= input_size { break; }

                    let w_val = match *weight_rc.borrow() {
                        RuntimeValue::Float(f) => f,
                        _ => 0.0
                    };

                    input_gradient[j] += w_val * d_val;
                }
            }
        }

        let result_objs: Vec<Rc<RefCell<RuntimeValue>>> = input_gradient
            .into_iter()
            .map(|f| Rc::new(RefCell::new(RuntimeValue::Float(f))))
            .collect();

        Ok(RuntimeValue::Register(result_objs))
    }

    fn builtin_time(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if !args.is_empty() {
            return Err("Runtime Error: 'time' expects 0 arguments.".to_string());
        }
        let start = std::time::SystemTime::now();
        let since_the_epoch = start
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("Time Error: {}", e))?;
        Ok(RuntimeValue::Float(since_the_epoch.as_secs_f64()))
    }

    fn builtin_matrix_update(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 4 && args.len() != 6 {
            return Err("Runtime Error: 'matrix_update' requires 4 or 6 args".to_string());
        }

        let lr = match args[3] {
            RuntimeValue::Float(f) => f,
            _ => return Err("Runtime Error: Learning rate must be a float".to_string())
        };

        let delta_vec = match &args[1] {
            RuntimeValue::Register(v) => v,
            _ => return Err("Runtime Error: Delta must be an array".to_string())
        };
        let input_vec = match &args[2] {
            RuntimeValue::Register(v) => v,
            _ => return Err("Runtime Error: Input must be an array".to_string())
        };

        let weights_matrix = match &args[0] {
            RuntimeValue::Register(v) => v,
            _ => return Err("Runtime Error: Weights must be a matrix".to_string())
        };

        // weights[i][j] -= lr * delta[i] * input[j]
        for (i, row_rc) in weights_matrix.iter().enumerate() {
            if i >= delta_vec.len() { break; }

            // Get delta[i]
            let d_val = match *delta_vec[i].borrow() {
                RuntimeValue::Float(f) => f,
                _ => 0.0
            };

            let row_val = row_rc.borrow();
            if let RuntimeValue::Register(row) = &*row_val {
                for (j, weight_rc) in row.iter().enumerate() {
                    if j >= input_vec.len() { break; }

                    // Get input[j]
                    let in_val = match *input_vec[j].borrow() {
                        RuntimeValue::Float(f) => f,
                        _ => 0.0
                    };

                    // Update Weight
                    let mut w_guard = weight_rc.borrow_mut();
                    if let RuntimeValue::Float(w) = *w_guard {
                        *w_guard = RuntimeValue::Float(w - (lr * d_val * in_val));
                    }
                }
            }
        }

        Ok(RuntimeValue::None)
    }

    fn builtin_debug_state(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err(
                "Runtime Error: 'debug_state' expects exactly 1 argument (a quantum register)."
                    .to_string(),
            );
        }
        match &args[0] {
            RuntimeValue::QuantumRegister { size, state, .. } => {
                Self::print_quantum_state(state, *size, 10);
                Ok(RuntimeValue::None)
            }
            _ => Err(format!(
                "Runtime Error: 'debug_state' argument must be a quantum register, got {}.",
                args[0].type_name()
            )),
        }
    }

    fn builtin_file_write(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 2 {
            return Err(
                "Runtime Error: 'file_write' expects 2 arguments (path, content).".to_string(),
            );
        }

        let path = match &args[0] {
            RuntimeValue::String(s) => s,
            _ => return Err("Runtime Error: File path must be a String.".to_string()),
        };

        let content = match &args[1] {
            RuntimeValue::String(s) => s,
            _ => return Err("Runtime Error: File content must be a String.".to_string()),
        };

        std::fs::write(path, content).map_err(|e| format!("File Write Error: {}", e))?;
        Ok(RuntimeValue::None)
    }

    fn builtin_file_read(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'file_read' expects 1 argument (path).".to_string());
        }

        let path = match &args[0] {
            RuntimeValue::String(s) => s,
            _ => return Err("Runtime Error: File path must be a String.".to_string()),
        };

        let content =
            std::fs::read_to_string(path).map_err(|e| format!("File Read Error: {}", e))?;
        Ok(RuntimeValue::String(content))
    }

    pub fn builtin_measure(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'measure' expects exactly one qubit.".to_string());
        }

        let (state_rc, target_index, reg_size) = match &args[0] {
            RuntimeValue::Qubit {
                state, index, size, ..
            } => (state, *index, *size),
            _ => return Err(format!("Runtime Error: Not a qubit.")),
        };
        Self::perform_measurement(state_rc, target_index, reg_size)
    }

    fn builtin_assert(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 2 {
            return Err(
                "Runtime Error: 'assert' expects 2 arguments (condition, message).".to_string(),
            );
        }

        let condition = match &args[0] {
            RuntimeValue::Bool(b) => *b,
            _ => {
                return Err(format!(
                    "Runtime Error: 'assert' argument 1 must be a Bool, got {}.",
                    args[0].type_name()
                ))
            }
        };

        let message = match &args[1] {
            RuntimeValue::String(s) => s.clone(),
            _ => {
                return Err(format!(
                    "Runtime Error: 'assert' argument 2 must be a String, got {}.",
                    args[1].type_name()
                ))
            }
        };

        if condition {
            Ok(RuntimeValue::None)
        } else {
            Err(format!("Assertion Failed: {}", message))
        }
    }
    fn builtin_print(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let output: Vec<String> = args
            .into_iter()
            .map(|val| match val {
                RuntimeValue::String(s) => s,
                RuntimeValue::Int(n) => n.to_string(),
                RuntimeValue::Float(f) => f.to_string(),
                RuntimeValue::Bool(b) => b.to_string(),
                RuntimeValue::None => "None".to_string(),
                _ => format!("{}", val),
            })
            .collect();
        println!("{}", output.join(" "));
        Ok(RuntimeValue::None)
    }
    fn builtin_split(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 2 {
            return Err(
                "Runtime Error: 'split' expects 2 arguments (string, delimiter).".to_string(),
            );
        }

        let target_str = match &args[0] {
            RuntimeValue::String(s) => s,
            _ => return Err("Runtime Error: First argument to split must be a String.".to_string()),
        };

        let delim = match &args[1] {
            RuntimeValue::String(s) => s,
            _ => return Err("Runtime Error: Delimiter must be a String.".to_string()),
        };

        let elements: Vec<std::rc::Rc<std::cell::RefCell<RuntimeValue>>> = target_str
            .split(delim)
            .map(|s| {
                let clean_str = s.trim().to_string();
                std::rc::Rc::new(std::cell::RefCell::new(RuntimeValue::String(clean_str)))
            })
            .collect();

        Ok(RuntimeValue::Register(elements))
    }
    fn builtin_input(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if let Some(val) = args.get(0) {
            print!("{}", val);
            let _ = std::io::stdout().flush();
        }

        let mut input_buffer = String::new();
        std::io::stdin()
            .read_line(&mut input_buffer)
            .map_err(|e| format!("Runtime Error: Failed to read input. {}", e))?;

        Ok(RuntimeValue::String(input_buffer.trim().to_string()))
    }

    fn builtin_to_int(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'to_int' expects exactly one argument.".to_string());
        }
        let val = &args[0];
        match val {
            RuntimeValue::Int(i) => Ok(RuntimeValue::Int(*i)),
            RuntimeValue::Float(f) => Ok(RuntimeValue::Int(*f as i64)),
            RuntimeValue::String(s) => s
                .parse::<i64>()
                .map(RuntimeValue::Int)
                .map_err(|_| format!("Runtime Error: Could not parse string '{}' as int.", s)),
            RuntimeValue::Bool(b) => Ok(RuntimeValue::Int(if *b { 1 } else { 0 })),
            _ => Err(format!(
                "Runtime Error: Cannot convert type {} to int.",
                val.type_name()
            )),
        }
    }
    fn builtin_len(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'len' expects exactly one argument.".to_string());
        }
        let val = &args[0];
        match val {
            RuntimeValue::String(s) => Ok(RuntimeValue::Int(s.len() as i64)),
            RuntimeValue::Register(arr) => Ok(RuntimeValue::Int(arr.len() as i64)),
            RuntimeValue::Dict(map) => Ok(RuntimeValue::Int(map.len() as i64)),
            RuntimeValue::QuantumRegister { size, .. } => Ok(RuntimeValue::Int(*size as i64)),
            _ => Err(format!(
                "Runtime Error: len() is not supported for type {}.",
                val.type_name()
            )),
        }
    }
    fn builtin_to_float(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'to_float' expects exactly one argument.".to_string());
        }
        let val = &args[0];
        match val {
            RuntimeValue::Int(i) => Ok(RuntimeValue::Float(*i as f64)),
            RuntimeValue::Float(f) => Ok(RuntimeValue::Float(*f)),
            RuntimeValue::String(s) => s
                .parse::<f64>()
                .map(RuntimeValue::Float)
                .map_err(|_| format!("Runtime Error: Could not parse string '{}' as float.", s)),
            _ => Err(format!(
                "Runtime Error: Cannot convert type {} to float.",
                val.type_name()
            )),
        }
    }
    fn builtin_echo(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'echo' expects exactly one argument.".to_string());
        }
        let value = args.into_iter().next().unwrap();
        println!("{}", value);
        Ok(value)
    }
    fn builtin_type_of(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'type_of' expects exactly one argument.".to_string());
        }
        Ok(RuntimeValue::String(args[0].type_name().to_string()))
    }
    fn builtin_to_string(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'to_string' expects exactly one argument.".to_string());
        }
        Ok(RuntimeValue::String(args[0].to_string()))
    }
    fn builtin_maybe(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 2 {
            return Err(
                "Runtime Error: 'maybe' expects two arguments: (value, confidence).".to_string(),
            );
        }
        let value = args.clone().into_iter().next().unwrap();
        let confidence_val = args.into_iter().nth(1).unwrap();
        let confidence = match confidence_val {
            RuntimeValue::Float(p) => {
                if !(0.0..=1.0).contains(&p) {
                    return Err(
                        "Runtime Error: Confidence must be between 0.0 and 1.0.".to_string()
                    );
                }
                p
            }
            _ => return Err("Runtime Error: Confidence (arg 2) must be a Float.".to_string()),
        };
        Ok(RuntimeValue::Probabilistic {
            value: Box::new(value),
            confidence,
        })
    }
    fn builtin_sample(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        if args.len() != 1 {
            return Err("Runtime Error: 'sample' expects exactly one argument.".to_string());
        }
        let prob_value = args.into_iter().next().unwrap();
        match prob_value {
            RuntimeValue::Probabilistic { value, confidence } => {
                if rand::thread_rng().gen::<f64>() < confidence {
                    Ok(*value)
                } else {
                    Ok(RuntimeValue::None)
                }
            }
            other_value => Ok(other_value),
        }
    }

    fn eval_let_declaration(
        name: &str,
        value_expr: &ASTNode,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let value = Self::evaluate(value_expr, env)?;
        env.borrow_mut().set(name.to_string(), value);
        Ok(RuntimeValue::None)
    }

    fn eval_assignment(
        target: &ASTNode,
        value_expr: &ASTNode,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let new_value = Self::evaluate(value_expr, env)?;

        match target {
            ASTNode::Identifier { name, .. } => {
                if let Some(var_rc) = env.borrow().get(name) {
                    *std::cell::RefCell::<_>::borrow_mut(&var_rc) = new_value;
                    Ok(RuntimeValue::None)
                } else {
                    Err(format!(
                        "Runtime Error: Cannot assign to undefined variable '{}'.",
                        name
                    ))
                }
            }

            ASTNode::ArrayAccess { array, index, loc } => {
                let collection_val = Self::evaluate(array, env)?;
                let index_val = Self::evaluate(index, env)?;

                match collection_val {
                    RuntimeValue::Dict(mut map) => {
                        let key = Self::value_to_string_key(index_val)?;
                        map.insert(key, Rc::new(RefCell::new(new_value)));
                        Ok(RuntimeValue::None)
                    }
                    RuntimeValue::Register(elements) => {
                        let index = match index_val {
                            RuntimeValue::Int(i) => i as usize,
                            _ => {
                                return Err(format!(
                                    "Runtime Error at {}: Array index must be an integer.",
                                    loc
                                ))
                            }
                        };

                        if index >= elements.len() {
                            return Err(format!("Runtime Error at {}: Array index {} out of bounds for array of size {}.", loc, index, elements.len()));
                        }

                        *elements[index].borrow_mut() = new_value;
                        Ok(RuntimeValue::None)
                    }
                    _ => Err(format!(
                        "Runtime Error at {}: Cannot perform subscript assignment on type {:?}",
                        loc,
                        collection_val.type_name()
                    )),
                }
            }

            ASTNode::MemberAccess { object, member } => {
                let object_val = Self::evaluate(object, env)?;

                match object_val {
                    RuntimeValue::Instance { fields, .. } => {
                        if let Some(field_rc) = fields.get(member) {
                            *field_rc.borrow_mut() = new_value;
                            Ok(RuntimeValue::None)
                        } else {
                            Err(format!(
                                "Runtime Error: Instance has no field named '{}'.",
                                member
                            ))
                        }
                    }
                    _ => Err(format!(
                        "Runtime Error: Cannot assign to member '{}' of non-instance type {}.",
                        member,
                        object_val.type_name()
                    )),
                }
            }

            _ => Err(
                "Runtime Error: Assignment target must be an identifier or subscript expression."
                    .to_string(),
            ),
        }
    }

    fn eval_identifier(
        name: &str,
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        if let Some(val_rc) = env.borrow().get(name) {
            Ok(val_rc.borrow().clone())
        } else {
            Err(format!(
                "Runtime Error at {}: Undefined variable '{}'",
                loc, name
            ))
        }
    }

    fn eval_import_statement(
        path: &ImportPath,
        alias: &str,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
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
                "Runtime Error: Failed to read module '{}' (checked local and install dirs): {}",
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

        let module_env = Rc::new(RefCell::new(Environment::new()));
        Self::register_builtins(&module_env);
        Self::evaluate_program(&ast, &module_env)?;

        let module_val = RuntimeValue::Module(module_env);

        env.borrow_mut().set(alias.to_string(), module_val);

        Ok(RuntimeValue::None)
    }

    fn eval_if_statement(
        condition: &Box<ASTNode>,
        then_block: &Box<ASTNode>,
        elif_blocks: &Vec<(ASTNode, ASTNode)>,
        else_block: &Option<Box<ASTNode>>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let cond_val = Self::evaluate(condition, env)?;
        if Self::is_truthy(&cond_val) {
            return Self::evaluate(then_block, env);
        }
        for (elif_cond_node, elif_body_node) in elif_blocks {
            let elif_cond_val = Self::evaluate(elif_cond_node, env)?;
            if Self::is_truthy(&elif_cond_val) {
                return Self::evaluate(elif_body_node, env);
            }
        }
        if let Some(else_body) = else_block {
            return Self::evaluate(else_body, env);
        }
        Ok(RuntimeValue::None)
    }

    fn eval_block(
        statements: &Vec<ASTNode>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        let mut last_result = RuntimeValue::None;
        for stmt in statements {
            last_result = Self::evaluate(stmt, env)?;
            match last_result {
                RuntimeValue::ReturnValue(_) | RuntimeValue::Break | RuntimeValue::Continue => {
                    return Ok(last_result)
                }
                _ => {}
            }
        }
        Ok(last_result)
    }

    fn eval_arguments(
        args: &[ASTNode],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Vec<RuntimeValue>, String> {
        let mut evaluated_args = Vec::new();
        for arg_expr in args {
            let arg_value = Self::evaluate(arg_expr, env)?;
            evaluated_args.push(arg_value);
        }
        Ok(evaluated_args)
    }

    fn value_to_string_key(value: RuntimeValue) -> Result<String, String> {
        match value {
        RuntimeValue::String(s) => Ok(s),
        RuntimeValue::Int(i) => Ok(i.to_string()),
        RuntimeValue::Bool(b) => Ok(b.to_string()),
        _ => Err(format!("Runtime Error: Invalid key type for dictionary. Must be String, Int, or Bool. Found {:?}", value.type_name())),
    }
    }
    fn eval_while_statement(
        condition: &Box<ASTNode>,
        body: &Box<ASTNode>,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        loop {
            let cond_val = Self::evaluate(condition, env)?;
            if !Self::is_truthy(&cond_val) {
                break;
            }
            let body_result = Self::evaluate(body, env)?;
            match body_result {
                RuntimeValue::Break => break,
                RuntimeValue::Continue => continue,
                RuntimeValue::ReturnValue(val) => return Ok(RuntimeValue::ReturnValue(val)),
                _ => {}
            }
        }
        Ok(RuntimeValue::None)
    }

    fn eval_binary_op(
        operator: &crate::parser::ast::BinaryOperator,
        left_expr: &Box<ASTNode>,
        right_expr: &Box<ASTNode>,
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        match operator {
            BinaryOperator::And => {
                let left_val = Self::evaluate(left_expr, env)?;
                if !Self::is_truthy(&left_val) {
                    return Ok(RuntimeValue::Bool(false));
                }
                let right_val = Self::evaluate(right_expr, env)?;
                return Ok(RuntimeValue::Bool(Self::is_truthy(&right_val)));
            }
            BinaryOperator::Or => {
                let left_val = Self::evaluate(left_expr, env)?;
                if Self::is_truthy(&left_val) {
                    return Ok(RuntimeValue::Bool(true));
                }
                let right_val = Self::evaluate(right_expr, env)?;
                return Ok(RuntimeValue::Bool(Self::is_truthy(&right_val)));
            }
            _ => {}
        }
        let left_val = Self::evaluate(left_expr, env)?;
        let right_val = Self::evaluate(right_expr, env)?;
        match (left_val.clone(), right_val.clone()) {
            (
                RuntimeValue::Probabilistic {
                    value: v1,
                    confidence: c1,
                },
                RuntimeValue::Probabilistic {
                    value: v2,
                    confidence: c2,
                },
            ) => {
                let inner_result = Self::eval_binary_op_runtime(operator, *v1, *v2, loc, env)?;
                Ok(RuntimeValue::Probabilistic {
                    value: Box::new(inner_result),
                    confidence: c1 * c2,
                })
            }
            (
                RuntimeValue::Probabilistic {
                    value: v1,
                    confidence: c1,
                },
                r_val,
            ) => {
                let inner_result = Self::eval_binary_op_runtime(operator, *v1, r_val, loc, env)?;
                Ok(RuntimeValue::Probabilistic {
                    value: Box::new(inner_result),
                    confidence: c1,
                })
            }
            (
                l_val,
                RuntimeValue::Probabilistic {
                    value: v2,
                    confidence: c2,
                },
            ) => {
                let inner_result = Self::eval_binary_op_runtime(operator, l_val, *v2, loc, env)?;
                Ok(RuntimeValue::Probabilistic {
                    value: Box::new(inner_result),
                    confidence: c2,
                })
            }
            (l_val, r_val) => Self::eval_binary_op_runtime(operator, l_val, r_val, loc, env),
        }
    }

    fn eval_binary_op_runtime(
        operator: &crate::parser::ast::BinaryOperator,
        left_val: RuntimeValue,
        right_val: RuntimeValue,
        loc: &Loc,
        _env: &Rc<RefCell<Environment>>,
    ) -> Result<RuntimeValue, String> {
        use crate::parser::ast::BinaryOperator::*;
        if matches!(operator, TensorProduct) {
            match (left_val.clone(), right_val.clone()) {
                (
                    RuntimeValue::QuantumRegister {
                        size: size_a,
                        state: state_a_rc,
                        ..
                    },
                    RuntimeValue::QuantumRegister {
                        size: size_b,
                        state: state_b_rc,
                        ..
                    },
                ) => {
                    let state_a = state_a_rc.borrow();
                    let state_b = state_b_rc.borrow();
                    let new_size = size_a + size_b;

                    let mut new_state = HashMap::new();

                    for (&i, &amp_a) in state_a.iter() {
                        for (&j, &amp_b) in state_b.iter() {
                            let new_index = (i << size_b) | j;
                            let new_amp_tuple = Self::complex_mul(amp_a, amp_b);

                            new_state.insert(new_index, new_amp_tuple);
                        }
                    }

                    return Ok(RuntimeValue::QuantumRegister {
                        size: new_size,
                        state: Rc::new(RefCell::new(new_state)),
                        register_name: "tensor_result".to_string(),
                        global_start_index: None,
                    });
                }

                (RuntimeValue::Register(vec_a), RuntimeValue::Register(vec_b)) => {
                    if vec_a.len() != vec_b.len() {
                        return Err("Dimension mismatch for dot product".to_string());
                    }

                    let mut sum = 0.0;
                    for (a_rc, b_rc) in vec_a.iter().zip(vec_b.iter()) {
                        let a = match *a_rc.borrow() {
                            RuntimeValue::Float(f) => f,
                            RuntimeValue::Int(i) => i as f64,
                            _ => return Err("Tensor product requires numbers".to_string()),
                        };
                        let b = match *b_rc.borrow() {
                            RuntimeValue::Float(f) => f,
                            RuntimeValue::Int(i) => i as f64,
                            _ => return Err("Tensor product requires numbers".to_string()),
                        };
                        sum += a * b;
                    }
                    return Ok(RuntimeValue::Float(sum));
                }

                (l, r) => {
                    return Err(format!(
                        "Runtime Error at {}: Operator {:?} not defined for types {:?} and {:?}",
                        loc,
                        operator,
                        l.type_name(),
                        r.type_name()
                    ))
                }
            }
        }
        match operator {
            Equal => {
                let result = match (&left_val, &right_val) {
                    (RuntimeValue::Int(l), RuntimeValue::Int(r)) => l == r,
                    (RuntimeValue::Float(l), RuntimeValue::Float(r)) => {
                        (l - r).abs() < f64::EPSILON
                    }
                    (RuntimeValue::Float(l), RuntimeValue::Int(r)) => {
                        (l - (*r as f64)).abs() < f64::EPSILON
                    }
                    (RuntimeValue::Int(l), RuntimeValue::Float(r)) => {
                        ((*l as f64) - r).abs() < f64::EPSILON
                    }
                    (RuntimeValue::String(l), RuntimeValue::String(r)) => l == r,
                    (RuntimeValue::Bool(l), RuntimeValue::Bool(r)) => l == r,
                    (RuntimeValue::None, RuntimeValue::None) => true,
                    _ => false,
                };
                return Ok(RuntimeValue::Bool(result));
            }
            NotEqual => {
                let result = match (&left_val, &right_val) {
                    (RuntimeValue::Int(l), RuntimeValue::Int(r)) => l != r,
                    (RuntimeValue::Float(l), RuntimeValue::Float(r)) => {
                        (l - r).abs() > f64::EPSILON
                    }
                    (RuntimeValue::Float(l), RuntimeValue::Int(r)) => {
                        (l - (*r as f64)).abs() > f64::EPSILON
                    }
                    (RuntimeValue::Int(l), RuntimeValue::Float(r)) => {
                        ((*l as f64) - r).abs() > f64::EPSILON
                    }
                    (RuntimeValue::String(l), RuntimeValue::String(r)) => l != r,
                    (RuntimeValue::Bool(l), RuntimeValue::Bool(r)) => l != r,
                    (RuntimeValue::None, RuntimeValue::None) => false,
                    _ => true,
                };
                return Ok(RuntimeValue::Bool(result));
            }
            Power => match (left_val, right_val) {
                (RuntimeValue::Int(base), RuntimeValue::Int(exp)) if exp >= 0 => {
                    match base.checked_pow(exp as u32) {
                        Some(result) => Ok(RuntimeValue::Int(result)),
                        None => Err(format!(
                            "Runtime Error at {}: Integer overflow in power operation",
                            loc
                        )),
                    }
                }
                (RuntimeValue::Int(base), RuntimeValue::Int(exp)) if exp < 0 => {
                    Ok(RuntimeValue::Float((base as f64).powf(exp as f64)))
                }
                (RuntimeValue::Float(base), RuntimeValue::Float(exp)) => {
                    Ok(RuntimeValue::Float(base.powf(exp)))
                }
                (RuntimeValue::Int(base), RuntimeValue::Float(exp)) => {
                    Ok(RuntimeValue::Float((base as f64).powf(exp)))
                }
                (RuntimeValue::Float(base), RuntimeValue::Int(exp)) => {
                    Ok(RuntimeValue::Float(base.powf(exp as f64)))
                }
                (l, r) => Err(format!(
                    "Runtime Error at {}: Power operator (^) not defined for types {:?} and {:?}",
                    loc,
                    l.type_name(),
                    r.type_name()
                )),
            },
            _ => {
                let (op, l, r) = match (operator, left_val, right_val) {
                    (Div, RuntimeValue::Int(l), RuntimeValue::Int(r)) => (
                        operator,
                        RuntimeValue::Float(l as f64),
                        RuntimeValue::Float(r as f64),
                    ),
                    (op, RuntimeValue::Int(l), RuntimeValue::Float(r))
                        if matches!(
                            op,
                            Add | Sub | Mul | Div | Less | Greater | LessEqual | GreaterEqual
                        ) =>
                    {
                        (op, RuntimeValue::Float(l as f64), RuntimeValue::Float(r))
                    }
                    (op, RuntimeValue::Float(l), RuntimeValue::Int(r))
                        if matches!(
                            op,
                            Add | Sub | Mul | Div | Less | Greater | LessEqual | GreaterEqual
                        ) =>
                    {
                        (op, RuntimeValue::Float(l), RuntimeValue::Float(r as f64))
                    }
                    (op, l, r) => (op, l, r),
                };
                match (op, l, r) {
                    (Add, RuntimeValue::Int(l), RuntimeValue::Int(r)) => {
                        Ok(RuntimeValue::Int(l + r))
                    }
                    (Add, RuntimeValue::String(l), RuntimeValue::String(r)) => {
                        Ok(RuntimeValue::String(format!("{}{}", l, r)))
                    }

                    (Add, RuntimeValue::Register(l_arr), RuntimeValue::Register(r_arr)) => {
                        let mut new_arr = l_arr.clone();
                        new_arr.extend(r_arr);
                        Ok(RuntimeValue::Register(new_arr))
                    }

                    (Sub, RuntimeValue::Int(l), RuntimeValue::Int(r)) => {
                        Ok(RuntimeValue::Int(l - r))
                    }
                    (Mul, RuntimeValue::Int(l), RuntimeValue::Int(r)) => {
                        Ok(RuntimeValue::Int(l * r))
                    }
                    (Mod, RuntimeValue::Int(l), RuntimeValue::Int(r)) => {
                        Ok(RuntimeValue::Int(l % r))
                    }
                    (
                        op @ (Add | Sub | Mul | Div),
                        RuntimeValue::Float(l),
                        RuntimeValue::Float(r),
                    ) => match op {
                        Div => {
                            if r == 0.0 {
                                Ok(RuntimeValue::Float(f64::NAN))
                            } else {
                                Ok(RuntimeValue::Float(l / r))
                            }
                        }
                        Add => Ok(RuntimeValue::Float(l + r)),
                        Sub => Ok(RuntimeValue::Float(l - r)),
                        Mul => Ok(RuntimeValue::Float(l * r)),
                        _ => unreachable!(),
                    },
                    (
                        op @ (Less | Greater | LessEqual | GreaterEqual),
                        RuntimeValue::Int(l),
                        RuntimeValue::Int(r),
                    ) => {
                        let result = match op {
                            Less => l < r,
                            Greater => l > r,
                            LessEqual => l <= r,
                            GreaterEqual => l >= r,
                            _ => unreachable!(),
                        };
                        Ok(RuntimeValue::Bool(result))
                    }
                    (
                        op @ (Less | Greater | LessEqual | GreaterEqual),
                        RuntimeValue::Float(l),
                        RuntimeValue::Float(r),
                    ) => {
                        let result = match op {
                            Less => l < r,
                            Greater => l > r,
                            LessEqual => l <= r,
                            GreaterEqual => l >= r,
                            _ => unreachable!(),
                        };
                        Ok(RuntimeValue::Bool(result))
                    }
                    (And, RuntimeValue::Bool(l), RuntimeValue::Bool(r)) => {
                        Ok(RuntimeValue::Bool(l && r))
                    }
                    (Or, RuntimeValue::Bool(l), RuntimeValue::Bool(r)) => {
                        Ok(RuntimeValue::Bool(l || r))
                    }
                    (op, l, r) => Err(format!(
                        "Runtime Error at {}: Operator {:?} not defined for types {:?} and {:?}",
                        loc,
                        op,
                        l.type_name(),
                        r.type_name()
                    )),
                }
            }
        }
    }
    fn builtin_graphics_create_canvas(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let width = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i as u32,
            _ => return Err("Width must be Int".to_string()),
        };
        let height = match args.get(1) {
            Some(RuntimeValue::Int(i)) => *i as u32,
            _ => return Err("Height must be Int".to_string()),
        };

        let mut id_guard = NEXT_ID.lock().unwrap();
        let id = *id_guard;
        *id_guard += 1;

        let canvas = Canvas::new(width, height);
        INTERPRETER_CANVASES.lock().unwrap().insert(id, canvas);

        Ok(RuntimeValue::Int(id))
    }

    fn builtin_graphics_set_background(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };
        let r = match args.get(1) {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 0,
        };
        let g = match args.get(2) {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 0,
        };
        let b = match args.get(3) {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 0,
        };
        let a = match args.get(4) {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 255,
        };

        let mut canvases = INTERPRETER_CANVASES.lock().unwrap();
        if let Some(canvas) = canvases.get_mut(&id) {
            canvas.set_background(Color::new(r, g, b, a));
        }
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_draw_rect(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };

        let to_f = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Float(f)) => *f,
            Some(RuntimeValue::Int(i)) => *i as f64,
            _ => 0.0,
        };
        let to_u8 = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 0,
        };

        let x = to_f(args.get(1));
        let y = to_f(args.get(2));
        let w = to_f(args.get(3));
        let h = to_f(args.get(4));
        let r = to_u8(args.get(5));
        let g = to_u8(args.get(6));
        let b = to_u8(args.get(7));
        let a = to_u8(args.get(8));
        let filled = match args.get(9) {
            Some(RuntimeValue::Int(i)) => *i != 0,
            _ => false,
        };

        let mut canvases = INTERPRETER_CANVASES.lock().unwrap();
        if let Some(canvas) = canvases.get_mut(&id) {
            canvas.draw_rectangle(x, y, w, h, Color::new(r, g, b, a), filled);
        }
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_draw_circle(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };

        let to_f = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Float(f)) => *f,
            Some(RuntimeValue::Int(i)) => *i as f64,
            _ => 0.0,
        };
        let to_u8 = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 0,
        };

        let x = to_f(args.get(1));
        let y = to_f(args.get(2));
        let radius = to_f(args.get(3));
        let r = to_u8(args.get(4));
        let g = to_u8(args.get(5));
        let b = to_u8(args.get(6));
        let a = to_u8(args.get(7));
        let filled = match args.get(8) {
            Some(RuntimeValue::Int(i)) => *i != 0,
            _ => false,
        };

        let mut canvases = INTERPRETER_CANVASES.lock().unwrap();
        if let Some(canvas) = canvases.get_mut(&id) {
            canvas.draw_circle(x, y, radius, Color::new(r, g, b, a), filled);
        }
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_draw_line(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };

        let to_f = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Float(f)) => *f,
            Some(RuntimeValue::Int(i)) => *i as f64,
            _ => 0.0,
        };
        let to_u8 = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 0,
        };

        let x1 = to_f(args.get(1));
        let y1 = to_f(args.get(2));
        let x2 = to_f(args.get(3));
        let y2 = to_f(args.get(4));
        let r = to_u8(args.get(5));
        let g = to_u8(args.get(6));
        let b = to_u8(args.get(7));
        let a = to_u8(args.get(8));
        let width = to_f(args.get(9)) as f32;

        let mut canvases = INTERPRETER_CANVASES.lock().unwrap();
        if let Some(canvas) = canvases.get_mut(&id) {
            canvas.draw_line(x1, y1, x2, y2, Color::new(r, g, b, a), width);
        }
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_draw_text(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };

        let to_f = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Float(f)) => *f,
            Some(RuntimeValue::Int(i)) => *i as f64,
            _ => 0.0,
        };
        let to_u8 = |v: Option<&RuntimeValue>| match v {
            Some(RuntimeValue::Int(i)) => *i as u8,
            _ => 0,
        };

        let x = to_f(args.get(1));
        let y = to_f(args.get(2));
        let text = match args.get(3) {
            Some(RuntimeValue::String(s)) => s.clone(),
            _ => "".to_string(),
        };
        let r = to_u8(args.get(4));
        let g = to_u8(args.get(5));
        let b = to_u8(args.get(6));
        let a = to_u8(args.get(7));
        let size = to_f(args.get(8)) as f32;

        let mut canvases = INTERPRETER_CANVASES.lock().unwrap();
        if let Some(canvas) = canvases.get_mut(&id) {
            canvas.draw_text(x, y, text, Color::new(r, g, b, a), size);
        }
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_save_svg(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };
        let filename = match args.get(1) {
            Some(RuntimeValue::String(s)) => s.clone(),
            _ => return Err("Filename error".to_string()),
        };

        let canvases = INTERPRETER_CANVASES.lock().unwrap();
        if let Some(canvas) = canvases.get(&id) {
            match canvas.save_svg(&filename) {
                Ok(_) => Ok(RuntimeValue::Int(0)),
                Err(e) => Err(format!("SVG Save Error: {}", e)),
            }
        } else {
            Err("Canvas ID not found".to_string())
        }
    }

    fn builtin_graphics_destroy_canvas(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };
        INTERPRETER_CANVASES.lock().unwrap().remove(&id);
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_create_plot(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let ptype_int = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => 0,
        };
        let ptype = match ptype_int {
            0 => PlotType::Line,
            1 => PlotType::Scatter,
            2 => PlotType::Bar,
            3 => PlotType::Histogram,
            4 => PlotType::Heatmap,
            _ => PlotType::Line,
        };

        let mut id_guard = NEXT_ID.lock().unwrap();
        let id = *id_guard;
        *id_guard += 1;

        let plot = Plot::new(ptype);
        INTERPRETER_PLOTS.lock().unwrap().insert(id, plot);
        Ok(RuntimeValue::Int(id))
    }

    fn builtin_graphics_plot_set_data(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };

        let extract_vec = |val: Option<&RuntimeValue>| -> Vec<f64> {
            match val {
                Some(RuntimeValue::Register(arr_rc)) => {
                    let mut res = Vec::new();
                    for item in arr_rc.iter() {
                        let inner = item.borrow();
                        if let RuntimeValue::Float(f) = *inner {
                            res.push(f);
                        } else if let RuntimeValue::Int(i) = *inner {
                            res.push(i as f64);
                        }
                    }
                    res
                }
                _ => Vec::new(),
            }
        };

        let x_data = extract_vec(args.get(1));
        let y_data = extract_vec(args.get(2));

        let mut plots = INTERPRETER_PLOTS.lock().unwrap();
        if let Some(plot) = plots.get_mut(&id) {
            plot.set_data(x_data, y_data);
        }
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_plot_set_title(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };
        let title = match args.get(1) {
            Some(RuntimeValue::String(s)) => s.clone(),
            _ => "".to_string(),
        };

        let mut plots = INTERPRETER_PLOTS.lock().unwrap();
        if let Some(plot) = plots.get_mut(&id) {
            plot.title = title;
        }
        Ok(RuntimeValue::None)
    }

    fn builtin_graphics_plot_render(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let plot_id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };
        let canvas_id = match args.get(1) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };

        let plots = INTERPRETER_PLOTS.lock().unwrap();
        let mut canvases = INTERPRETER_CANVASES.lock().unwrap();

        if let (Some(plot), Some(canvas)) = (plots.get(&plot_id), canvases.get_mut(&canvas_id)) {
            match plot.render_to_canvas(canvas) {
                Ok(_) => Ok(RuntimeValue::Int(0)),
                Err(e) => Err(format!("Plot Render Error: {}", e)),
            }
        } else {
            Err("Plot or Canvas not found".to_string())
        }
    }

    fn builtin_graphics_destroy_plot(args: Vec<RuntimeValue>) -> Result<RuntimeValue, String> {
        let id = match args.get(0) {
            Some(RuntimeValue::Int(i)) => *i,
            _ => return Err("ID error".to_string()),
        };
        INTERPRETER_PLOTS.lock().unwrap().remove(&id);
        Ok(RuntimeValue::None)
    }

    pub fn evaluate_program_with_lifecycle(
        node: &ASTNode,
        env: &Rc<RefCell<Environment>>,
        lifecycle: &mut QubitLifecycleManager,
        simulator: &mut NativeSimulator,
    ) -> Result<RuntimeValue, String> {
        let mut qubit_map: QubitMap = HashMap::new();
        let mut next_alloc = 0;

        Self::eval_recursive(
            node,
            env,
            lifecycle,
            simulator,
            &mut qubit_map,
            &mut next_alloc,
        )
    }
    fn resolve_qubits(
        args: &[ASTNode],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Vec<usize>, String> {
        let mut indices = Vec::new();
        for arg in args {
            let val = Self::evaluate(arg, env)?;
            if let RuntimeValue::Qubit { global_index, .. } = val {
                if let Some(idx) = global_index {
                    indices.push(idx);
                } else {
                    return Err(
                        "Runtime Error: Qubit has no hardware index (Simulator not active?)."
                            .to_string(),
                    );
                }
            } else {
                return Err("Runtime Error: Argument is not a Qubit.".to_string());
            }
        }
        Ok(indices)
    }

    fn eval_recursive(
        node: &ASTNode,
        env: &Rc<RefCell<Environment>>,
        lifecycle: &mut QubitLifecycleManager,
        simulator: &mut NativeSimulator,
        qubit_map: &mut QubitMap,
        next_alloc: &mut usize,
    ) -> Result<RuntimeValue, String> {
        match node {
            ASTNode::Program(statements) | ASTNode::Block(statements) => {
                let mut last = RuntimeValue::None;
                for stmt in statements {
                    last = Self::eval_recursive(
                        stmt, env, lifecycle, simulator, qubit_map, next_alloc,
                    )?;
                    if let RuntimeValue::ReturnValue(_)
                    | RuntimeValue::Break
                    | RuntimeValue::Continue = last
                    {
                        return Ok(last);
                    }
                }
                Ok(last)
            }

            ASTNode::QuantumDeclaration { name, size, .. } => {
                let count = if let Some(size_expr) = size {
                    match Self::evaluate(size_expr, env)? {
                        RuntimeValue::Int(n) => n as usize,
                        _ => 1,
                    }
                } else {
                    1
                };

                qubit_map.insert(name.clone(), *next_alloc);

                lifecycle.register_qubits(name, count, QubitState::Classical(false));

                let dummy_state = Self::default_state_vector(count)?;
                let register = RuntimeValue::QuantumRegister {
                    size: count,
                    state: Rc::new(RefCell::new(dummy_state)),
                    register_name: name.clone(),
                    global_start_index: Some(*next_alloc),
                };

                env.borrow_mut().set(name.clone(), register);
                *next_alloc += count;
                Ok(RuntimeValue::None)
            }

            ASTNode::Apply {
                gate_expr,
                arguments,
                loc,
            } => {
                let qubit_ids = Self::extract_qubit_ids_runtime(arguments, env)?;
                let (gate_name, is_dagger) = Self::extract_gate_info_runtime(gate_expr)?;

                if Self::is_controlled_gate(gate_expr) {
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

                let target_indices = Self::resolve_qubits(arguments, env)?;

                let mut params = Vec::new();
                Self::extract_gate_params(gate_expr, &mut params, env)?;

                simulator
                    .apply_gate(&gate_name, &target_indices, &params, is_dagger)
                    .map_err(|e| format!("Simulator Error: {}", e))?;

                Ok(RuntimeValue::None)
            }

            ASTNode::Measure(qubit_expr) => {
                let qubit_ids = Self::extract_qubit_ids_runtime(&[*qubit_expr.clone()], env)?;
                let loc = Self::get_node_location(qubit_expr);
                for qubit_id in &qubit_ids {
                    lifecycle
                        .record_operation(qubit_id, QubitOperation::Measure, loc)
                        .map_err(|e| e.to_string())?;
                }

                let indices = Self::resolve_qubits(&[*qubit_expr.clone()], env)?;
                if indices.is_empty() {
                    return Err("Runtime Error: Measure target invalid.".to_string());
                }

                let result = simulator.measure_single(indices[0]);
                Ok(RuntimeValue::Int(result as i64))
            }

            ASTNode::Block(statements) => {
                let mut last = RuntimeValue::None;
                for stmt in statements {
                    last = Self::eval_recursive(
                        stmt, env, lifecycle, simulator, qubit_map, next_alloc,
                    )?;
                    match last {
                        RuntimeValue::ReturnValue(_)
                        | RuntimeValue::Break
                        | RuntimeValue::Continue => return Ok(last),
                        _ => {}
                    }
                }
                Ok(last)
            }

            ASTNode::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                let cond_val = Self::evaluate(condition, env)?;
                if Self::is_truthy(&cond_val) {
                    return Self::eval_recursive(
                        then_block, env, lifecycle, simulator, qubit_map, next_alloc,
                    );
                }
                for (cond, body) in elif_blocks {
                    if Self::is_truthy(&Self::evaluate(cond, env)?) {
                        return Self::eval_recursive(
                            body, env, lifecycle, simulator, qubit_map, next_alloc,
                        );
                    }
                }
                if let Some(else_body) = else_block {
                    return Self::eval_recursive(
                        else_body, env, lifecycle, simulator, qubit_map, next_alloc,
                    );
                }
                Ok(RuntimeValue::None)
            }

            ASTNode::While { condition, body } => {
                loop {
                    if !Self::is_truthy(&Self::evaluate(condition, env)?) {
                        break;
                    }
                    let res = Self::eval_recursive(
                        body, env, lifecycle, simulator, qubit_map, next_alloc,
                    )?;
                    match res {
                        RuntimeValue::Break => break,
                        RuntimeValue::Continue => continue,
                        RuntimeValue::ReturnValue(v) => return Ok(RuntimeValue::ReturnValue(v)),
                        _ => {}
                    }
                }
                Ok(RuntimeValue::None)
            }

            ASTNode::For {
                variable,
                iterator,
                body,
            } => {
                let iter_val = Self::evaluate(iterator, env)?;
                let iterable: Vec<RuntimeValue> = match iter_val {
                    RuntimeValue::Range(r) => r.into_iter().map(RuntimeValue::Int).collect(),
                    RuntimeValue::Register(r) => r.iter().map(|v| v.borrow().clone()).collect(),
                    _ => return Err("Invalid iterator".to_string()),
                };
                for item in iterable {
                    env.borrow_mut().set(variable.clone(), item);
                    let res = Self::eval_recursive(
                        body, env, lifecycle, simulator, qubit_map, next_alloc,
                    )?;
                    match res {
                        RuntimeValue::Break => break,
                        RuntimeValue::Continue => continue,
                        RuntimeValue::ReturnValue(v) => return Ok(RuntimeValue::ReturnValue(v)),
                        _ => {}
                    }
                }
                Ok(RuntimeValue::None)
            }

            ASTNode::LetDeclaration { name, value, .. } => {
                let val =
                    Self::eval_recursive(value, env, lifecycle, simulator, qubit_map, next_alloc)?;
                env.borrow_mut().set(name.clone(), val);
                Ok(RuntimeValue::None)
            }

            ASTNode::Assignment { target, value } => {
                let new_value =
                    Self::eval_recursive(value, env, lifecycle, simulator, qubit_map, next_alloc)?;
                match &**target {
                    ASTNode::Identifier { name, .. } => {
                        if let Some(var_rc) = env.borrow().get(name) {
                            *var_rc.borrow_mut() = new_value;
                            Ok(RuntimeValue::None)
                        } else {
                            Err(format!("Runtime Error: Undefined variable '{}'.", name))
                        }
                    }
                    _ => Self::eval_assignment(target, value, env),
                }
            }

            ASTNode::FunctionCall {
                callee,
                arguments,
                loc,
                is_dagger,
            } => Self::eval_function_call_with_lifecycle_recursive(
                callee, arguments, loc, env, *is_dagger, lifecycle, simulator, qubit_map,
                next_alloc,
            ),

            _ => Self::evaluate(node, env),
        }
    }

    fn eval_function_call_with_lifecycle_recursive(
        callee_expr: &ASTNode,
        arguments: &[ASTNode],
        loc: &Loc,
        env: &Rc<RefCell<Environment>>,
        is_dagger: bool,
        lifecycle: &mut QubitLifecycleManager,
        simulator: &mut NativeSimulator,
        qubit_map: &mut QubitMap,
        next_alloc: &mut usize,
    ) -> Result<RuntimeValue, String> {
        if let ASTNode::Identifier { name, .. } = callee_expr {
            if name == "debug_state" {
                if let Some(arg) = arguments.get(0) {
                    let mut indices = Vec::new();

                    match arg {
                        ASTNode::Identifier { name: var_name, .. } => {
                            if let Some(&start) = qubit_map.get(var_name) {
                                if let Some(val) = env.borrow().get(var_name) {
                                    if let RuntimeValue::QuantumRegister { size, .. } =
                                        &*val.borrow()
                                    {
                                        for i in 0..*size {
                                            indices.push(start + i);
                                        }
                                    }
                                }
                            }
                        }
                        ASTNode::ArrayAccess { array, index, .. } => {
                            if let ASTNode::Identifier { name: var_name, .. } = &**array {
                                if let ASTNode::IntLiteral(idx) = &**index {
                                    if let Some(&start) = qubit_map.get(var_name) {
                                        indices.push(start + (*idx as usize));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }

                    if !indices.is_empty() {
                        simulator.dump_state(&indices);
                        return Ok(RuntimeValue::None);
                    }
                }
            }
        }

        let evaluated_args = Self::eval_arguments(arguments, env)?;
        let function = Self::evaluate(callee_expr, env)?;

        match function {
            RuntimeValue::Function {
                parameters,
                body,
                env: func_env,
            } => {
                let mut function_scope = Environment::new_enclosed(func_env);
                if parameters.len() != evaluated_args.len() {
                    return Err(format!("Runtime Error: Arg count mismatch at {}", loc));
                }
                for (param, arg_val) in parameters.iter().zip(evaluated_args) {
                    function_scope.set(param.name.clone(), arg_val);
                }
                let function_scope_rc = Rc::new(RefCell::new(function_scope));

                let result = Self::eval_recursive(
                    &body,
                    &function_scope_rc,
                    lifecycle,
                    simulator,
                    qubit_map,
                    next_alloc,
                )?;

                if let RuntimeValue::ReturnValue(val) = result {
                    Ok(*val)
                } else {
                    Ok(result)
                }
            }
            _ => Self::eval_function_call(callee_expr, arguments, loc, env, is_dagger),
        }
    }

    fn extract_qubit_ids_runtime(
        arguments: &[ASTNode],
        env: &Rc<RefCell<Environment>>,
    ) -> Result<Vec<QubitId>, String> {
        let mut ids = Vec::new();
        for arg in arguments {
            let val = Self::evaluate(arg, env)?;

            if let RuntimeValue::Qubit {
                index,
                register_name,
                ..
            } = val
            {
                ids.push(QubitId::new(register_name, index));
            }
        }
        Ok(ids)
    }

    fn extract_gate_name_runtime(gate_expr: &ASTNode) -> Result<String, String> {
        match gate_expr {
            ASTNode::Gate { name, .. } => Ok(name.to_lowercase()),
            ASTNode::ParameterizedGate { name, .. } => Ok(name.to_lowercase()),
            ASTNode::Controlled { gate_expr, .. } => Self::extract_gate_name_runtime(gate_expr),
            ASTNode::Dagger { gate_expr, .. } => Self::extract_gate_name_runtime(gate_expr),
            _ => Err("Invalid gate expression".to_string()),
        }
    }
    fn is_controlled_gate(gate_expr: &ASTNode) -> bool {
        match gate_expr {
            ASTNode::Controlled { .. } => true,
            ASTNode::Dagger {
                gate_expr: inner, ..
            } => Self::is_controlled_gate(inner),
            _ => false,
        }
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
            ASTNode::Apply { loc, .. } => *loc,
            ASTNode::Measure(inner) => Self::get_node_location(inner),
            _ => Loc { line: 0, column: 0 },
        }
    }
}
