// src/parser/ast.rs
use std::fmt;
use std::collections::HashMap;
#[derive(Debug, Clone, PartialEq,Copy)]
pub struct Loc {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[Line {}, Col {}]", self.line, self.column)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportPath {
    File(String),        // For: "./my_math.qc"
    Module(Vec<String>), // For: ai.neural
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportSpec {
    List(Vec<String>), // For 'import name1, name2'
    All,              // For 'import *'
}


#[derive(Debug, Clone, PartialEq)]
pub enum ASTNode {
    // Program root
    Program(Vec<ASTNode>),
    
    // Statements
    LetDeclaration {
        
        name: String,
        type_annotation: Option<Type>,
        value: Box<ASTNode>,
        is_mutable: bool,
    },

    Import {
        path: ImportPath, // e.g., "./my_math.qc"
        alias: String, // e.g., "math"
    },

    FromImport {
        path: ImportPath, // e.g., "./my_math.qc"
        spec: ImportSpec, // e.g., ["add", "PI"]
    },

    TryCatch {
        try_block: Box<ASTNode>,
        error_variable: Option<String>, // For 'catch err:'
        catch_block: Box<ASTNode>,
    },
    
    QuantumDeclaration {
        name: String,
        size: Option<Box<ASTNode>>, // For arrays like quantum q[5]
        initial_state: Option<Box<ASTNode>>,
    },
    FunctionDeclaration {
        
        name: String,
        parameters: Vec<Parameter>,
        return_type: Option<Type>,
        body: Box<ASTNode>,
    },
    CircuitDeclaration {
        
        name: String,
        parameters: Vec<Parameter>,
        return_type: Option<Type>,
        body: Box<ASTNode>,
    },
    Return(Option<Box<ASTNode>>),
    
    // Control flow
    If {
        condition: Box<ASTNode>,
        then_block: Box<ASTNode>,
        elif_blocks: Vec<(ASTNode, ASTNode)>,
        else_block: Option<Box<ASTNode>>,
    },
    Match {
        value: Box<ASTNode>,
        cases: Vec<(Pattern, ASTNode)>,
    },
    For {
        variable: String,
        iterator: Box<ASTNode>,
        body: Box<ASTNode>,
    },
    While {
        condition: Box<ASTNode>,
        body: Box<ASTNode>,
    },
    Break,
    Continue,
    
    // Expressions
    Binary {
        operator: BinaryOperator,
        left: Box<ASTNode>,
        right: Box<ASTNode>,
        loc: Loc
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<ASTNode>,
    },
    FunctionCall {
        callee: Box<ASTNode>,
        arguments: Vec<ASTNode>,
        loc: Loc,
        is_dagger: bool,
    },
    Apply {
        gate_expr: Box<ASTNode>,
        arguments: Vec<ASTNode>,
        loc: Loc,
    },
    Gate {
        name: String,
        loc: Loc,
        
    },

    ParameterizedGate {
        name: String,
        parameters: Vec<ASTNode>,
        loc: Loc,
    },
    Dagger {
        gate_expr: Box<ASTNode>,
        loc: Loc,
    },
    Controlled {
        gate_expr: Box<ASTNode>,
        loc: Loc,
    },
    Measure(Box<ASTNode>),
    
    // Literals
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    BoolLiteral(bool),
    NoneLiteral,
    QuantumKet(String),  // |0}, |1}, |+}, etc.
    QuantumBra(String),  // {0|, {1|, etc.
    
    // Variables and access
    Identifier {
        name: String,
        loc: Loc, // NEW: Store the location of the identifier
    },
    ArrayAccess {
        array: Box<ASTNode>,
        index: Box<ASTNode>,
        loc: Loc,
    },
    MemberAccess {
        object: Box<ASTNode>,
        member: String,
    },
    
    // Collections
    ArrayLiteral(Vec<ASTNode>),
    DictLiteral(Vec<(ASTNode, ASTNode)>),
    Range {
        start: Box<ASTNode>,
        end: Box<ASTNode>,
        inclusive: bool,
    },
    
    // Block
    Block(Vec<ASTNode>),
    
    // Assignment
    Assignment {
        target: Box<ASTNode>,
        value: Box<ASTNode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Add, Sub, Mul, Div, Mod,
    Equal, NotEqual, Less, Greater, LessEqual, GreaterEqual,
    And, Or,
    TensorProduct,  // ***
    Pipeline, 
    Power,      
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Not,
    Minus,
    Plus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int, Int8, Int16, Int32, Int64, Int128,
    Uint, Uint8, Uint16, Uint32, Uint64, Uint128,
    Float, Float32, Float64,
    Complex, Complex64, Complex128,
    Bool,
    Bit,
    String,
    Qubit,
    QuantumRegister(Option<usize>),
    QuantumArray(Box<Type>, Option<usize>),
    Tensor(Box<Type>, Vec<Option<usize>>),
    Array(Box<Type>),
    Dict,
    Module(HashMap<String, Type>),
    Function(Vec<Type>, Box<Type>),
    Custom(String),
    Any,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal(ASTNode),
    Identifier(String),
    Wildcard,

}
