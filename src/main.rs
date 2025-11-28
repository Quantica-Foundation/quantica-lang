// src/main.rs
mod lexer;
mod parser;
mod environment;
mod evaluator;
mod type_checker;
mod doc_generator;
mod codegen;
mod linker;
mod runtime;

use crate::environment::Environment;
use crate::evaluator::Evaluator;
use crate::type_checker::TypeChecker;
use crate::type_checker::TypeEnvironment;
use lexer::Lexer;
use parser::Parser;
use std::env;
use std::fs;
use std::path::Path; 
use parser::ast::ImportSpec;
use parser::ast::ImportPath;
use std::io::{self, Write};
use crate::parser::ast::ASTNode;
use crate::environment::RuntimeValue;
use crate::doc_generator::DocGenerator;
use crate::lexer::token::Token;
use inkwell::context::Context;
use crate::codegen::Compiler;
use crate::linker::Linker;
use inkwell::OptimizationLevel;

#[derive(Debug, Clone, Copy, PartialEq)]
enum CompilationTarget {
    HostCPU,  // Default (LLVM/X86)
    SPIRV,    // Vulkan/OpenCL GPU
    XLA,      // TPU/Specialized Accelerator
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Quantica Compiler v0.1 ===\n");
    
    let args: Vec<String> = env::args().collect();

    let mut show_ast = false;
    let mut show_tokens = false;
    let mut verbose=false;
    let mut filename: Option<&str> = None;
    let mut emit_llvm = false;
    let mut command: Option<&str> = None;
    let mut opt_level = OptimizationLevel::Default;
    let mut enable_lto = false;
    let mut target = CompilationTarget::HostCPU;


    for arg in args.iter().skip(1) {

        if arg == "--ast" {                   
            show_ast = true;
        } else if arg == "--tokens" {        
            show_tokens = true;
        }else if arg == "--v"{
            verbose=true;
        } else if arg == "--emit-llvm" {
            emit_llvm = true;
        } else if arg == "--lto" { 
            enable_lto = true;
        
        }else if arg.starts_with("--target") { 
            let parts: Vec<&str> = arg.split('=').collect();
            if parts.len() == 2 {
                target = match parts[1].to_lowercase().as_str() {
                    "spirv" => CompilationTarget::SPIRV,
                    "xla"   => CompilationTarget::XLA,
                    "host"  => CompilationTarget::HostCPU,
                    _ => return Err(format!("Unknown target '{}'. Use spirv, xla, or host.", parts[1]).into()),
                };
            }
        } else if arg.starts_with("-O") { 
            opt_level = parse_opt_level(arg)?; 
        } else if arg == "--doc" || arg == "--repl" || arg == "--test" || arg == "--lex" || arg == "--compile" || arg == "--run" {
            command = Some(arg);
        } else if filename.is_none() {
            filename = Some(arg);
        }
    }


    if command.is_none() && filename.is_none() {
        println!("Starting REPL mode (type '.quit' to exit, '.clear' to reset).");
        run_repl();
        return Ok(());
    }
    
   
    if args.len() < 2 {
        println!("Starting REPL mode (type '.quit' to exit, '.clear' to reset).");
        run_repl(); 
        return Ok(());
    }

    if args[1] == "--version" {
        println!("Quantica v0.1.0");
        println!("Quantica By Quantica Foundation");
        return Ok(());
    }

    if args[1] == "--help" || args[1] == "-h" {
        print_help();
        return Ok(());
    }

    if args[1] == "--doc" && args.len() >= 3 {
        let filename = &args[2];
        let output_file = "docs/api.md"; 
        println!("📄 Generating documentation for: {}", filename);
        println!("   Outputting to: {}", output_file);
        
        match run_doc_generator(filename, output_file) {
            Ok(()) => {
                println!("✓ Documentation generated successfully!");
            }
            Err(e) => {
                eprintln!("✗ DocGen Error: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    
    if args[1] == "--repl" {
        println!("Starting REPL mode (type '.quit' to exit, '.clear' to reset).");
        run_repl();
        return Ok(());
    }

    if args[1] == "--test" {
        run_test_suite();
        return Ok(());
    }
    
    
    if args[1] == "--lex" && args.len() >= 3 {
        run_lexer_only(&args[2]);
        return Ok(());
    }

    if args[1] == "--compile" && args.len() >= 3 {
        let filename = &args[2];
        let object_file = "output.o";
        
        
        let exe_name = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("program");
        
        let output_exe = if cfg!(target_os = "windows") {
            format!("{}.exe", exe_name)
        } else {
            exe_name.to_string()
        };
        
        println!("🚀 Compiling to executable: {} -> {}", filename, output_exe);
        
        match compile_file_llvm(filename, object_file,emit_llvm,opt_level,enable_lto,target) {
            Ok(()) => {
                println!("✓ LLVM compilation successful!\n");
                
                
                let runtime_lib_dir = "target/debug";
                match Linker::link_executable(object_file, &output_exe, runtime_lib_dir,enable_lto) {
                    Ok(()) => {

                        fs::remove_file(object_file)
                            .map_err(|e| format!("Failed to clean up object file: {}", e))?;
                        println!("\n🎉 Build successful!");
                        println!("   Run your program with: ./{}", output_exe);
                    }
                    Err(e) => {
                        eprintln!("\n✗ Linking failed: {}", e);
                        eprintln!("\nℹ️  You can try manual linking:");
                        eprintln!("   1. Open 'x64 Native Tools Command Prompt for VS'");
                        eprintln!("   2. Run: link {} target\\debug\\quantica.lib /OUT:{} /SUBSYSTEM:CONSOLE kernel32.lib msvcrt.lib", object_file, output_exe);
                        fs::remove_file(object_file).unwrap_or_default();
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("\n✗ Compilation failed: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }


    if args[1] == "--run" && args.len() >= 3 {
        let filename = &args[2];
        println!("🚀 Running JIT Compiler: {}", filename);
        match run_jit_file(filename,emit_llvm,opt_level,target) {
            Ok(()) => {
                println!("\n✓ JIT execution successful!");
            }
            Err(e) => {
                eprintln!("\n✗ Execution failed: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    

    if let Some(file) = filename {
        compile_file(file, show_ast, show_tokens, verbose);
    } else {
        eprintln!("Error: No input file specified");
        eprintln!("Usage: quantica [options] <file.qc>");
        eprintln!("Options:");
        eprintln!("  -v, --verbose    Show compilation phases");
        eprintln!("  --ast            Show Abstract Syntax Tree");
        eprintln!("  --tokens         Show token stream");
        eprintln!("  --emit-llvm  Emit LLVM IR");
        eprintln!("  --compile    Compile to executable");
        eprintln!("  --run        JIT compile and run");
        eprintln!("  --doc        Generate documentation");
        eprintln!("  --repl       Start interactive REPL");
        eprintln!("  --test       Run test suite");
        std::process::exit(1);
    }

    Ok(())
}

fn compile_file(filename: &str,show_ast: bool, show_tokens: bool,verbose:bool) {
    
    if verbose {
        println!("📄 Compiling: {}\n", filename);
    }
   
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("✗ Failed to read file '{}': {}", filename, e);
            std::process::exit(1);
        }
    };
    if verbose {
        println!("Source code:");
        println!("{:-<60}", "");
        println!("{}", source);
        println!("{:-<60}\n", "");
    }

    if verbose{
        println!("🔤 Phase 1: Lexical Analysis");
    }
    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => {
            if verbose {
                println!("✓ Lexer succeeded! {} tokens\n", t.len());
            }
            t
        }
        Err(e) => {
            eprintln!("✗ Lexer error: {}", e);
            std::process::exit(1);
        }
    };

    if show_tokens {
        println!("=== TOKENS ===");
        for (i, token) in tokens.iter().enumerate() {
            if !matches!(token.token, Token::Newline | Token::Indent | Token::Dedent) {
                println!("{:4}: {:?}", i, token.token);
            }
        }
        println!("==============\n");
    }
    

    if verbose{
        println!("🌳 Phase 2: Syntax Analysis");
    }
    let mut parser = Parser::new(tokens);
    let ast = match parser.parse() {
        Ok(tree) => {
            if verbose {
                println!("✓ Parser succeeded!\n");
            }    
            tree
        }
        Err(e) => {
            eprintln!("✗ Parser error: {}", e);
            std::process::exit(1);
        }
    };
    if verbose{
        println!("🔬 Phase 2.5: Type Checking");
    }
    match TypeChecker::check_program(&ast) {
        Ok(()) => {
            if verbose{
                println!("✓ Type check succeeded!\n");
            }
        }
        Err(e) => {
            eprintln!("✗ Type error: {}", e);
            std::process::exit(1);
        }
    }
    
  
    
    if show_ast {
        println!("=== ABSTRACT SYNTAX TREE ===");
        print_ast(&ast, 0);
        println!("============================\n");
    }
    
    
    
  
    if verbose{
        println!("✨ Phase 3: Interpretation");
    }
   
    let env = std::rc::Rc::new(std::cell::RefCell::new(Environment::new()));

    if verbose{
        println!("Program Output:");
        println!("{:-<60}", "");
    }

    let evaluation_result = Evaluator::evaluate_program(&ast, &env);
    
    println!("{:-<60}", "");
    
    match evaluation_result {
        Ok(_) => {
            println!("✓ Execution successful!");
        }
        Err(e) => {
            eprintln!("✗ Runtime Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!("Quantica Compiler v0.1.0");
    println!();
    println!("USAGE:");
    println!("    quantica [OPTIONS] <file.qc>");
    println!();
    println!("OPTIONS:");
    println!("    --version,        Show version information");
    println!("    --help, -h           Show this help message");
    println!("    --compile <file>     Compile to native executable");
    println!("    --run <file>         JIT compile and run");
    println!("    --doc <file>         Generate API documentation");
    println!("    --repl               Start interactive REPL");
    println!("    --test               Run test suite");
    println!("    --lex <file>         Tokenize only");
    println!();
    println!("COMPILATION OPTIONS:");
    println!("    --ast                Show Abstract Syntax Tree");
    println!("    --tokens             Show token stream");
    println!("    --emit-llvm          Emit LLVM IR");
    println!("    --lto                Enable Link-Time Optimization");
    println!("    -O0, -O1, -O2, -O3   Set optimization level");
    println!("    --target=<target>    Compilation target (host, spirv, xla)");
    println!();
    println!("EXAMPLES:");
    println!("    quantica hello.qc             # Run a Quantica program");
    println!("    quantica --compile app.qc     # Compile to executable");
    println!("    quantica --repl               # Start REPL");
    println!("    quantica --doc lib.qc         # Generate documentation");
}
fn compile_file_llvm(filename: &str, output_file: &str, emit_llvm: bool, opt_level: OptimizationLevel,enable_lto: bool,target: CompilationTarget) -> Result<(), String> {
    println!("📄 Compiling: {}\n", filename);

   
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;

  
    println!("🔤 Phase 1: Lexical Analysis");
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;
    
 
    println!("🌳 Phase 2: Syntax Analysis");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse()
        .map_err(|e| format!("Parser error: {}", e))?;

 
    println!("🔬 Phase 3: Type Checking");
    TypeChecker::check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    println!("✓ Type check succeeded!\n");

 

 
    println!("🤖 Phase 4: Code Generation (LLVM)");
    let context = Context::create();
    let mut compiler = Compiler::new(&context,opt_level);


    compiler.compile_program(&ast)?;
    println!("   -> Generated LLVM IR");


    compiler.optimize_module(opt_level)?;
    println!("   -> Optimized module");

    compiler.finalize_debug_info();
    println!("   -> Finalized debug info");

    let module_name = Path::new(filename).file_stem().and_then(|s| s.to_str()).unwrap_or("quantica_program");
    
 
    let hlo_ir = compiler.export_to_hlo_ir(module_name)?;
    println!("   -> XLA/HLO IR Exported");
    
    
    if emit_llvm {
        println!("\n--- GENERATED LLVM IR (Optimized) ---");
        compiler.dump_ir();
        println!("-------------------------------------\n");
        
     
        println!("--- XLA/HLO IR (TPU Target) ---");
        println!("{}", hlo_ir);
        println!("-------------------------------\n");
    }

    if target != CompilationTarget::HostCPU {
      
        println!("⚠️ Target is {:?}. Skipping AOT linking and outputting IR only.", target);
        
    
        let module_name = Path::new(filename).file_stem().and_then(|s| s.to_str()).unwrap_or("quantica_program");
        let hlo_ir = compiler.export_to_hlo_ir(module_name)?;

        if emit_llvm {
            println!("--- XLA/HLO IR (TPU Target) ---");
            println!("{}", hlo_ir);
            println!("-------------------------------\n");
        }
        return Ok(());
    }
    
    
    compiler.write_to_object_file(output_file)?;
    println!("   -> Emitted object file");

    

    Ok(())
}


fn run_jit_file(filename: &str, emit_llvm: bool, opt_level: OptimizationLevel,target: CompilationTarget) -> Result<(), String> {
    println!("📄 JIT Compiling and Running: {}\n", filename);

 
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;

    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;
    
    let mut parser = Parser::new(tokens);
    let ast = parser.parse()
        .map_err(|e| format!("Parser error: {}", e))?;


    TypeChecker::check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    if target == CompilationTarget::SPIRV {
        println!("🚀 Accelerating with SPIR-V/GPU Backend...");
    } else if target == CompilationTarget::XLA {
        println!("🚀 Accelerating with XLA/TPU Backend...");
    }
    
   
    println!("⚙️ Phase 3: Compiling for JIT...");

   
    let jit_program = if let ASTNode::Program(ref nodes) = ast {
      
        let has_main = nodes.iter().any(|node| {
            matches!(node, ASTNode::FunctionDeclaration { name, .. } if name == "main")
        });
        
        if has_main {
       
            ast
        } else {
     
            let main_function_node = ASTNode::FunctionDeclaration {
                
                name: "main".to_string(),
                parameters: Vec::new(),
                return_type: None,
                body: Box::new(ASTNode::Block(nodes.clone())),
            };
            ASTNode::Program(vec![main_function_node])
        }
    } else {
        return Err("Expected ASTNode::Program at root.".to_string());
    };


    println!("🤖 Phase 4: JIT Compilation & Execution");
    let context = Context::create();
    let mut compiler = Compiler::new(&context,opt_level);

    let function_names = if let ASTNode::Program(s) = &jit_program {
        s.iter().filter_map(|stmt| {
            if let ASTNode::FunctionDeclaration { name, .. } = stmt {
                Some(name.clone())
            } else {
                None
            }
        }).collect()
    } else {
        Vec::new()
    };

    compiler.enable_jit_profiling(function_names)?;

    
    compiler.compile_jit_program(&jit_program)?;

    compiler.finalize_debug_info();

    if emit_llvm {
        println!("\n--- GENERATED LLVM IR (JIT) ---");
        compiler.dump_ir();
        println!("-------------------------------\n");
    } 
    
    println!("✨ Program Output:");
    println!("{:-<60}", "");
    compiler.run_jit()?;
    println!("{:-<60}", "");

    Ok(())
}


fn run_test_suite() {
    println!("=== Running Quantica Test Suite ===\n");
    let test_dir = "tests/";
    let mut passed_count = 0;
    let mut failed_count = 0;

    match fs::read_dir(test_dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("✗ Failed to read test directory entry: {}", e);
                        continue;
                    }
                };
                
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "qc" {
                            let filename = path.to_str().unwrap_or("unknown file");
                            print!("Running test: {} ... ", filename);
                            
                         
                            match run_test_file(filename) {
                                Ok(()) => {
                                    println!("PASS ✅");
                                    passed_count += 1;
                                }
                                Err(e) => {
                                    println!("FAIL ❌");
                                    eprintln!("  Error: {}\n", e);
                                    failed_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to read test directory '{}': {}", test_dir, e);
            eprintln!("  Please create a 'tests/' directory and add .qc test files.");
            std::process::exit(1);
        }
    }

    // Print summary
    println!("\n{:-<60}", "");
    println!("Test Summary:");
    println!("  ✅ Passed: {}", passed_count);
    println!("  ❌ Failed: {}", failed_count);
    println!("{:-<60}", "");

    if failed_count > 0 {
        std::process::exit(1);
    }
}


fn run_test_file(filename: &str) -> Result<(), String> {
    

    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;
    
  
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;
    
   
    let mut parser = Parser::new(tokens);
    let ast = parser.parse()
        .map_err(|e| format!("Parser error: {}", e))?;
    
   
    TypeChecker::check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
 
    let env = std::rc::Rc::new(std::cell::RefCell::new(Environment::new()));
    Evaluator::evaluate_program(&ast, &env)
        .map_err(|e| format!("Runtime Error: {}", e))?; 

 
    Ok(())
}




fn print_ast(node: &ASTNode, indent: usize) {
    let prefix = "  ".repeat(indent);
    
    match node {
        ASTNode::Program(statements) => {
            println!("{}Program:", prefix);
            for stmt in statements {
                print_ast(stmt, indent + 1);
            }
        }
        ASTNode::LetDeclaration { .. } => { 
             println!("{}LetDeclaration", prefix);
             
        }
        ASTNode::QuantumDeclaration { name, size, initial_state } => {
            let size_str = if let Some(_s) = size {
                format!("[size]")
            } else {
                String::new()
            };
            println!("{}Quantum {}{}", prefix, name, size_str);
            if let Some(state) = initial_state {
                print_ast(state, indent + 1);
            }
        }

        ASTNode::Import { path, alias } => {
            let path_str = match path {
                ImportPath::File(f) => format!("\"{}\"", f),
                ImportPath::Module(m) => m.join("."),
            };
            println!("{}Import {} as {}", prefix, path_str, alias);
        }
        
        ASTNode::FunctionDeclaration { name, parameters, return_type, body , .. } => {
            let ret_str = if let Some(t) = return_type {
                format!(" -> {:?}", t)
            } else {
                String::new()
            };
            println!("{}Function {}({} params){}", prefix, name, parameters.len(), ret_str);
            print_ast(body, indent + 1);
        }
        ASTNode::CircuitDeclaration { name, parameters, return_type, body, .. } => {
            let ret_str = if let Some(t) = return_type {
                format!(" -> {:?}", t)
            } else {
                String::new()
            };
            println!("{}Circuit {}({} params){}", prefix, name, parameters.len(), ret_str);
            print_ast(body, indent + 1);
        }
        ASTNode::Return(value) => {
            println!("{}Return:", prefix);
            if let Some(v) = value {
                print_ast(v, indent + 1);
            }
        }
        ASTNode::If { condition, then_block, elif_blocks, else_block } => {
            println!("{}If:", prefix);
            println!("{}  Condition:", prefix);
            print_ast(condition, indent + 2);
            println!("{}  Then:", prefix);
            print_ast(then_block, indent + 2);
            for (i, (cond, body)) in elif_blocks.iter().enumerate() {
                println!("{}  Elif {}:", prefix, i);
                print_ast(cond, indent + 2);
                print_ast(body, indent + 2);
            }
            if let Some(else_body) = else_block {
                println!("{}  Else:", prefix);
                print_ast(else_body, indent + 2);
            }
        }
        ASTNode::For { variable, iterator, body } => {
            println!("{}For {} in:", prefix, variable);
            print_ast(iterator, indent + 1);
            print_ast(body, indent + 1);
        }
        ASTNode::While { condition, body } => {
            println!("{}While:", prefix);
            print_ast(condition, indent + 1);
            print_ast(body, indent + 1);
        }
        ASTNode::Binary { operator, left, right, .. } => {
            println!("{}Binary {:?}:", prefix, operator);
            print_ast(left, indent + 1);
            print_ast(right, indent + 1);
        }
        
        ASTNode::ParameterizedGate { ref name, ref parameters, loc: _, ..} => {
            println!("{}Parameterized gate: {} with params {:?}", prefix, name, parameters);
        },
        ASTNode::Unary { operator, operand } => {
            println!("{}Unary {:?}:", prefix, operator);
            print_ast(operand, indent + 1);
        }
        ASTNode::FunctionCall { callee, arguments, .. } => {
            println!("{}FunctionCall({} args)", prefix, arguments.len());
            println!("{}  Callee:", prefix);
            print_ast(callee, indent + 2);
            println!("{}  Arguments:", prefix);
            for arg in arguments {
                print_ast(arg, indent + 2);
            }
        }
        ASTNode::Apply { gate_expr, arguments, .. } => {
            println!("{}Apply ({} args):", prefix, arguments.len());
            println!("{}  Gate:", prefix);
            print_ast(gate_expr, indent + 2);
            println!("{}  Args:", prefix);
            for arg in arguments {
                print_ast(arg, indent + 2);
            }
        }
        ASTNode::Gate { name, .. } => {
            println!("{}Gate: {}", prefix, name);
        }
        ASTNode::Dagger { gate_expr, .. } => {
            println!("{}Dagger:", prefix);
            print_ast(gate_expr, indent + 1);
        }
        ASTNode::Controlled { gate_expr, .. } => {
            println!("{}Controlled:", prefix);
            print_ast(gate_expr, indent + 1);
        }
        ASTNode::Measure(qubit) => {
            println!("{}Measure:", prefix);
            print_ast(qubit, indent + 1);
        }
        ASTNode::IntLiteral(n) => {
            println!("{}Int: {}", prefix, n);
        }
        ASTNode::FloatLiteral(f) => {
            println!("{}Float: {}", prefix, f);
        }
        ASTNode::StringLiteral(s) => {
            println!("{}String: \"{}\"", prefix, s);
        }
        ASTNode::BoolLiteral(b) => {
            println!("{}Bool: {}", prefix, b);
        }
        ASTNode::NoneLiteral => {
            println!("{}None", prefix);
        }
        ASTNode::QuantumKet(state) => {
            println!("{}|{}}}", prefix, state);
        }
        ASTNode::QuantumBra(state) => {
            println!("{}{{{}|", prefix, state);
        }
        ASTNode::Identifier { name, .. } => {
            println!("{}Id: {}", prefix, name);
        }
        ASTNode::ArrayAccess { array, index, .. } => {
            println!("{}ArrayAccess:", prefix);
            print_ast(array, indent + 1);
            println!("{}  [index]:", prefix);
            print_ast(index, indent + 2);
        }
        ASTNode::MemberAccess { object, member } => {
            println!("{}MemberAccess .{}:", prefix, member);
            print_ast(object, indent + 1);
        }
        ASTNode::ArrayLiteral(elements) => {
            println!("{}Array[{}]:", prefix, elements.len());
            for elem in elements {
                print_ast(elem, indent + 1);
            }
        }
        ASTNode::DictLiteral(pairs) => {
            println!("{}Dict{{{}}}:", prefix, pairs.len());
            for (key, value) in pairs {
                println!("{}  Key:", prefix);
                print_ast(key, indent + 2);
                println!("{}  Value:", prefix);
                print_ast(value, indent + 2);
            }
        }
        ASTNode::Range { start, end, inclusive } => {
            let op = if *inclusive { "..=" } else { ".." };
            println!("{}Range {}:", prefix, op);
            print_ast(start, indent + 1);
            print_ast(end, indent + 1);
        }
        ASTNode::Block(statements) => {
            println!("{}Block:", prefix);
            for stmt in statements {
                print_ast(stmt, indent + 1);
            }
        }
        ASTNode::Assignment { target, value } => {
            println!("{}Assignment:", prefix);
            println!("{}  Target:", prefix);
            print_ast(target, indent + 2);
            println!("{}  Value:", prefix);
            print_ast(value, indent + 2);
        }
        ASTNode::Match { value, cases } => {
            println!("{}Match:", prefix);
            print_ast(value, indent + 1);
            for (pattern, body) in cases {
                println!("{}  Case {:?}:", prefix, pattern);
                print_ast(body, indent + 2);
            }
        }
        ASTNode::Break => {
            println!("{}Break", prefix);
        }
        ASTNode::FromImport { path, spec } => {
            let path_str = match path {
                ImportPath::File(f) => format!("\"{}\"", f),
                ImportPath::Module(m) => m.join("."),
            };
            println!("{}FromImport {}:", prefix, path_str);
            
            match spec {
                ImportSpec::All => {
                    println!("{}  - * (All)", prefix);
                }
                ImportSpec::List(names) => {
                    for name in names {
                        println!("{}  - {}", prefix, name);
                    }
                }
            }
        }
        ASTNode::Continue => {
            println!("{}Continue", prefix);
        }
        ASTNode::TryCatch { try_block, error_variable, catch_block } => {
            println!("{}Try:", prefix);
            print_ast(try_block, indent + 1);

            if let Some(var) = error_variable {
                println!("{}Catch ({}):", prefix, var);
            } else {
                println!("{}Catch:", prefix);
            }

            print_ast(catch_block, indent + 1);
        }
    }
}

fn run_lexer_only(filename: &str) {
    println!("🔤 Tokenizing: {}\n", filename);
    
    let source = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("✗ Failed to read file '{}': {}", filename, e);
            std::process::exit(1);
        }
    };
    
    let mut lexer = Lexer::new(&source);
    
    match lexer.tokenize() {
        Ok(tokens) => {
            println!("✓ Lexer succeeded! Found {} tokens\n", tokens.len());
            println!("{:-<60}", "");
            
            let important_tokens: Vec<_> = tokens.iter()
                .filter(|t| !matches!(t.token, 
                    Token::Newline | 
                    Token::Indent | 
                    Token::Dedent))
                .collect();
            
            for (i, token) in important_tokens.iter().enumerate() {
                println!("{:4}: Line {:3}, Col {:3} - {:?}", 
                    i, 
                    token.line, 
                    token.column, 
                    token.token
                );
            }
            
            println!("{:-<60}", "");
            println!("\n✓ Tokenization complete!");
        }
        Err(e) => {
            eprintln!("\n✗ Lexer error: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_repl() {
  
    let runtime_env = std::rc::Rc::new(std::cell::RefCell::new(Environment::new()));
    let type_env = std::rc::Rc::new(std::cell::RefCell::new(TypeEnvironment::new()));
  
    TypeChecker::prefill_environment(&type_env);

    let mut source_buffer = String::new();
    let mut is_continuation = false;

    loop {
        
        let prompt = if is_continuation { ".. " } else { ">> " };
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            println!("Error reading line.");
            return;
        }

        let trimmed_line = line.trim();

        
        if !is_continuation {
            if trimmed_line == ".quit" || trimmed_line == ".exit" {
                break;
            }
            if trimmed_line == ".clear" {
                source_buffer.clear();
                is_continuation = false;
                println!("Buffer cleared.");
                continue;
            }
        }

 
        source_buffer.push_str(&line);
        if source_buffer.trim().is_empty() {
            continue;
        }

      
        
     
        let mut lexer = Lexer::new(&source_buffer);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(e) if e.contains("Unterminated") => {
               
                is_continuation = true;
                continue;
            }
            Err(e) => {
             
                println!("Lexer Error: {}", e);
                source_buffer.clear();
                is_continuation = false;
                continue;
            }
        };

     
        let mut parser = Parser::new(tokens.clone());
        let ast = match parser.parse() {
            Ok(tree) => tree,
            Err(e) if e.contains("Unexpected EOF") || e.contains("Expected Indent") || e.contains("Dedent") => {
                
                is_continuation = true;
                continue;
            }
            Err(e) => {
           
                println!("Parser Error: {}", e);
                source_buffer.clear();
                is_continuation = false;
                continue;
            }
        };

    
        if let ASTNode::Program(statements) = ast {
            for stmt in statements {
            
                let type_check_result = TypeChecker::check(&stmt, &type_env, None);
                if let Err(e) = type_check_result {
                    println!("Type Error: {}", e);
                
                    break; 
                }

               
                let eval_result = Evaluator::evaluate(&stmt, &runtime_env);
                match eval_result {
                    Ok(RuntimeValue::None) => { /* Don't print None */ }
                    Ok(value) => {
                        println!("{}", value);
                    }
                    Err(e) => {
                        println!("Runtime Error: {}", e);
                      
                        break;
                    }
                }
            }
        }

       
        source_buffer.clear();
        is_continuation = false;
    }
}
fn run_doc_generator(filename: &str, output_file: &str) -> Result<(), String> {
    

    println!("   -> Step 1: Reading source file...");
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;
    
  
    println!("   -> Step 2: Lexing tokens...");
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize() 
        .map_err(|e| format!("Lexer error: {}", e))?;
    
 
    println!("   -> Step 3: Parsing AST...");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse() 
        .map_err(|e| format!("Parser error: {}", e))?;
    
   
    println!("   -> Step 4: Generating docs...");
    DocGenerator::run(&ast, output_file)?;
    
    println!("   -> Step 5: Done.");
    Ok(())
}

fn parse_opt_level(arg: &str) -> Result<OptimizationLevel, String> {
    match arg {
        "-O0" => Ok(OptimizationLevel::None),
        "-O1" => Ok(OptimizationLevel::Less),
        "-O2" => Ok(OptimizationLevel::Default),
        "-O3" => Ok(OptimizationLevel::Aggressive),
        _ => Err(format!("Unknown optimization flag: {}. Use -O0, -O1, -O2, or -O3.", arg)),
    }
}

//Made by M.Gurukasi from Quantica Foundation

