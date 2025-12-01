// src/lib.rs

pub mod lexer;
pub mod parser;
pub mod environment;
pub mod evaluator;
pub mod type_checker;
pub mod doc_generator;
pub mod codegen;
pub mod runtime;
pub mod quantum_backend;
pub mod hardware_integration;
pub use runtime::{
    quantica_rt_new_state,
    quantica_rt_debug_state,
    quantica_rt_apply_gate,
    quantica_rt_measure,
};

pub mod linker;

