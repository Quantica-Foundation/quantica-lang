#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Token {
    // Keywords
    Let,
    Print,
    Echo,
    Mut,
    Auto,
    Func,
    Circuit,
    Class,
    Return,
    Import,
    From,
    Use,
    As,
    Module,
    Package,
    
    // Control Flow
    If,
    Elif,
    Else,
    Match,
    Case,
    For,
    In,
    While,
    Break,
    Continue,
    
    // Error & Safety
    Try,
    Catch,
    Except,
    Finally,
    Throw,
    Safe,
    
    // Parallelism
    Parallel,
    Task,
    Await,
    Async,
    Yield,
    
    // Logical Keywords
    And,
    Or,
    Not,
    Is,
    
    // Quantum
    Quantum,
    Apply,
    Measure,
    Dagger,
    Controlled,

    //quantum gates
    Hadamard,
    Cnot,
    X,
    Y,
    Z,
    S,
    T,
    Swap,
    Reset,
    CZ,
    CS,
    CT,
    CPhase,
    U,
    CCX,
    Toffoli,
    RX,
    RY,
    RZ,
    
    
    // AI/ML
    Tensor,
    Train,
    Infer,
    Load,
    Save,
    
    // Data Structures
    Struct,
    Enum,
    
    // Modifiers
    Const,
    Public,
    Private,
    Static,
    Extern,
    Inline,
    
    // Meta
    Pragma,
    Sizeof,
    Typeof,
    
    // Future Reserved
    Defer,
    Contract,
    Where,
    Generic,
    
    // Types
    Int, Int8, Int16, Int32, Int64, Int128,
    Uint, Uint8, Uint16, Uint32, Uint64, Uint128,
    Float, Float32, Float64,
    Complex, Complex64, Complex128,
    Bool,
    Bit,
    String,
    
    // Literals
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    True,
    False,
    None,
    Any,
    
    // Identifiers
    Identifier(String),
    
    
    // Operators
    Plus,           // +
    Minus,          // -
    Star,           // *
    Slash,          // /
    Percent,        // %
    Equal,          // =
    ColonEqual,     // :=
    EqualEqual,     // ==
    NotEqual,       // !=
    Less,           // 
    Greater,        // >
    LessEqual,      // <=
    GreaterEqual,   // >=
    Question,       // ?
    Bang,           // !
    
    // Punctuation
    LeftParen,      // (
    RightParen,     // )
    LeftBracket,    // [
    RightBracket,   // ]
    LeftBrace,      // {
    RightBrace,     // }
    Comma,          // ,
    Dot,            // .
    Colon,          // :
    Semicolon,      // ;
    Arrow,          // ->
    FatArrow,       // =>
    DoubleColon,    // ::
    Range,          // ..
    RangeInclusive, // ..=
    SafeNav,        // ?.
    Pipe,           // |
    DoublePipe,     // ||
    PipeRight,      // |>
    PipeDouble,     // =>>
    Caret, //^
    TensorProduct,  // *** (quantum tensor product)
    
    // Quantum Notation
    KetState(String),  // |0}, |1}, |+}, |-}
    BraState(String),  // {0|, {1|
    
    // Special
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TokenWithLocation {
    pub token: Token,
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

impl TokenWithLocation {
    pub fn new(token: Token, line: usize, column: usize, length: usize) -> Self {
        Self { token, line, column, length }
    }

}

