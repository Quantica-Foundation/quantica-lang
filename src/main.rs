/*  src/main.rs */
mod codegen;
mod doc_generator;
mod environment;
mod error;
mod error_bridge;
mod error_codes;
mod evaluator;
mod lexer;
mod linker;
mod parser;
mod qubit_lifecycle;
mod runtime;
mod type_checker;

mod graphics;
mod graphics_runtime;
mod hardware_integration;
mod quantum_backend;
use crate::quantum_backend::native_simulator::NativeSimulator;
use crate::qubit_lifecycle::QubitLifecycleManager;
use hardware_integration::{parse_hardware_config, HardwareExecutor};
use quantum_backend::QuantumConfig;

use crate::codegen::Compiler;
use crate::doc_generator::DocGenerator;
use crate::environment::Environment;
use crate::environment::RuntimeValue;
use crate::error::{Diagnostic, ErrorCategory, ErrorReporter};
use crate::error_bridge::report_string_error;
use crate::evaluator::Evaluator;
use crate::lexer::token::Token;
use crate::linker::Linker;
use crate::parser::ast::ASTNode;
use crate::parser::ast::Loc;
use crate::type_checker::TypeChecker;
use crate::type_checker::TypeEnvironment;
use inkwell::context::Context;
use inkwell::OptimizationLevel;
use lexer::Lexer;
use parser::ast::ImportPath;
use parser::ast::ImportSpec;
use parser::Parser;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
enum CompilationTarget {
    HostCPU, // Default (LLVM/X86)
    SPIRV,   // Vulkan/OpenCL GPU
    XLA,     // TPU/Specialized Accelerator
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Quantica Compiler v0.2.1 ===\n");

    let args: Vec<String> = env::args().collect();

    let mut show_ast = false;
    let mut show_tokens = false;
    let mut verbose = false;
    let mut emit_llvm = false;
    let mut command: Option<&str> = None;
    let mut opt_level = OptimizationLevel::Default;
    let mut enable_lto = false;
    let mut target = CompilationTarget::HostCPU;
    let mut hardware_config: Option<QuantumConfig> = None;
    let mut list_devices = false;
    let mut enable_lifecycle_checking = true;

    if args.contains(&"--hardware".to_string()) {
        hardware_config = parse_hardware_config(&args);
    }
    for arg in &args {
        if arg == "--no-lifecycle" {
            enable_lifecycle_checking = false;
        }
    }

    if enable_lifecycle_checking {
        println!("Note : Qubit lifecycle checking enabled");
    } else {
        println!("⚠️ Qubit lifecycle checking disabled");
    }

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--hardware" => {
                i += 2;
            }
            "--device" | "--shots" | "--api-token" => {
                i += 2;
            }
            "--no-optimize" => {
                i += 1;
            }
            "--list-devices" => {
                list_devices = true;
                i += 1;
            }
            "--ast" => {
                show_ast = true;
                i += 1;
            }
            "--tokens" => {
                show_tokens = true;
                i += 1;
            }
            "--v" => {
                verbose = true;
                i += 1;
            }
            "--emit-llvm" => {
                emit_llvm = true;
                i += 1;
            }
            "--lto" => {
                enable_lto = true;
                i += 1;
            }
            _ if args[i].starts_with("--target=") => {
                let parts: Vec<&str> = args[i].split('=').collect();
                if parts.len() == 2 {
                    target = match parts[1].to_lowercase().as_str() {
                        "spirv" => CompilationTarget::SPIRV,
                        "xla" => CompilationTarget::XLA,
                        "host" => CompilationTarget::HostCPU,
                        _ => return Err(format!("Unknown target '{}'", parts[1]).into()),
                    };
                }
                i += 1;
            }
            _ if args[i].starts_with("-O") => {
                opt_level = parse_opt_level(&args[i])?;
                i += 1;
            }
            _ if args[i] == "--doc"
                || args[i] == "--repl"
                || args[i] == "--test"
                || args[i] == "--lex"
                || args[i] == "--compile"
                || args[i] == "--run" =>
            {
                command = Some(&args[i]);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let filename = hardware_integration::extract_filename(&args);

    if list_devices {
        if let Some(config) = hardware_config {
            use quantum_backend::BackendManager;
            let backend_manager = BackendManager::new(config);
            let devices = backend_manager.list_devices();
            println!("📡 Available devices:");
            for device in devices {
                println!("   • {}", device);
            }
            return Ok(());
        } else {
            eprintln!("Error: --list-devices requires --hardware <provider>");
            std::process::exit(1);
        }
    }

    // Handle hardware execution mode
    if let Some(config) = hardware_config {
        if let Some(file) = filename {
            println!("🚀 Executing on Quantum Hardware: {:?}\n", config.provider);
            println!("📄 File: {}", file);
            println!(
                "🎯 Device: {}",
                config
                    .device_name
                    .as_ref()
                    .unwrap_or(&"default".to_string())
            );
            println!("🎲 Shots: {}\n", config.shots);
            return run_on_hardware(&file, config);
        } else {
            eprintln!("Error: No input file specified for hardware execution");
            eprintln!("Usage: quantica --hardware native <filename.qc>");
            std::process::exit(1);
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

    if args.len() >= 2 && args[1] == "--version" {
        println!("Quantica v0.1.2");
        println!("Quantica By Quantica Foundation");
        return Ok(());
    }

    if args.len() >= 2 && (args[1] == "--help" || args[1] == "-h") {
        print_help();
        return Ok(());
    }

    if args.len() >= 2 && args[1] == "--doc" && args.len() >= 3 {
        let fname = &args[2];
        let output_file = "docs/api.md";
        println!("📄 Generating documentation for: {}", fname);
        println!("   Outputting to: {}", output_file);

        match run_doc_generator(fname, output_file) {
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

    if args.len() >= 2 && args[1] == "--repl" {
        println!("Starting REPL mode (type '.quit' to exit, '.clear' to reset).");
        run_repl();
        return Ok(());
    }

    if args.len() >= 2 && args[1] == "--test" {
        run_test_suite();
        return Ok(());
    }

    if args.len() >= 2 && args[1] == "--lex" && args.len() >= 3 {
        run_lexer_only(&args[2]);
        return Ok(());
    }

    if args.len() >= 2 && args[1] == "--compile" && args.len() >= 3 {
        let fname = &args[2];
        let object_file = "output.o";

        let exe_name = Path::new(fname)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("program");

        let output_exe = if cfg!(target_os = "windows") {
            format!("{}.exe", exe_name)
        } else {
            exe_name.to_string()
        };

        println!("🚀 Compiling to executable: {} -> {}", fname, output_exe);

        match compile_file_llvm(fname, object_file, emit_llvm, opt_level, enable_lto, target) {
            Ok(()) => {
                println!("✓ LLVM compilation successful!\n");

                let runtime_lib_dir = "target/debug";
                match Linker::link_executable(object_file, &output_exe, runtime_lib_dir, enable_lto)
                {
                    Ok(()) => {
                        fs::remove_file(object_file)
                            .map_err(|e| format!("Failed to clean up object file: {}", e))?;
                        println!("\n🎉 Build successful!");
                        println!("   Run your program with: ./{}", output_exe);
                    }
                    Err(e) => {
                        eprintln!("\n✗ Linking failed: {}", e);
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

    if args.len() >= 2 && args[1] == "--run" && args.len() >= 3 {
        let fname = &args[2];
        println!("🚀 Running JIT Compiler: {}", fname);
        match run_jit_file(fname, emit_llvm, opt_level, target) {
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

    // Full compilation pipeline: Lex + Parse
    if let Some(file) = filename {
        compile_file(&file, show_ast, show_tokens, verbose);
    } else {
        eprintln!("Error: No input file specified");
        eprintln!("Usage: quantica [options] <file.qc>");
        eprintln!("\nTry: quantica --help");
        std::process::exit(1);
    }

    Ok(())
}

fn compile_file(filename: &str, show_ast: bool, show_tokens: bool, verbose: bool) {
    if verbose {
        println!("📄 Compiling: {}\n", filename);
    }

    // Read source
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

    let _reporter = ErrorReporter::new(&source, filename);

    // Lexical Analysis
    if verbose {
        println!("🔤 Phase 1: Lexical Analysis");
    }
    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            report_string_error(e, &source, filename);
            std::process::exit(1);
        }
    };
    //lexer.debug_print_tokens(&tokens);

    if show_tokens {
        println!("=== TOKENS ===");
        for (i, token) in tokens.iter().enumerate() {
            if !matches!(token.token, Token::Newline | Token::Indent | Token::Dedent) {
                println!("{:4}: {:?}", i, token.token);
            }
        }
        println!("==============\n");
    }

    // Syntax Analysis (Parsing)
    if verbose {
        println!("🌳 Phase 2: Syntax Analysis");
    }
    let mut parser = Parser::new(tokens);
    let ast = match parser.parse() {
        Ok(tree) => tree,
        Err(e) => {
            report_string_error(e, &source, filename);
            std::process::exit(1);
        }
    };

    if verbose {
        println!("🔬 Phase 2.5: Type Checking");
    }

    match TypeChecker::check_program(&ast) {
        Ok(()) => {}
        Err(e) => {
            report_string_error(e, &source, filename);
            std::process::exit(1);
        }
    }

    // Display AST (Optional)
    if show_ast {
        println!("=== ABSTRACT SYNTAX TREE ===");
        print_ast(&ast, 0);
        println!("============================\n");
    }

    if verbose {
        println!("✨ Phase 3: Interpretation");
    }

    let env = std::rc::Rc::new(std::cell::RefCell::new(Environment::new()));
    Evaluator::register_builtins(&env);
    let mut lifecycle = QubitLifecycleManager::new(true); // strict mode enabled

    let total_qubits = count_total_qubits(&ast);
    if verbose {
        println!("   -> Allocated Quantum Memory: {} qubits", total_qubits);
    }

    let mut simulator = NativeSimulator::new(total_qubits);

    if verbose {
        println!("Program Output:");
        println!("{:-<60}", "");
    }

    let start_time = Instant::now();

    //Evaluator (Hybrid Mode)
    let result =
        Evaluator::evaluate_program_with_lifecycle(&ast, &env, &mut lifecycle, &mut simulator);

    let duration = start_time.elapsed();

    println!("{:-<60}", "");
    println!("Interpreter Time: {:.6} seconds", duration.as_secs_f64());

    match result {
        Ok(_) => {
            println!("✓ Execution successful!");

            if verbose {
                println!("\n=== Qubit Lifecycle Summary ===");
                lifecycle.print_summary();
            }
        }
        Err(e) => {
            report_string_error(e, &source, filename);
            std::process::exit(1);
        }
    }
}

fn extract_variable_name(msg: &str) -> String {
    if let Some(start) = msg.find('\'') {
        if let Some(end) = msg[start + 1..].find('\'') {
            return msg[start + 1..start + 1 + end].to_string();
        }
    }
    "variable".to_string()
}

fn print_help() {
    println!("Quantica Compiler v0.1.0");
    println!();
    println!("USAGE:");
    println!("    quantica [OPTIONS] <file.qc>");
    println!();
    println!("OPTIONS:");
    println!("    --version            Show version information");
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
    println!("QUANTUM HARDWARE OPTIONS:");
    println!("    --hardware <provider>    Run on quantum hardware/simulator");
    println!("                             Providers:");
    println!(
        "                             - native   : Native Rust simulator (default, fast, no deps)"
    );
    println!("                             - ibm      : IBM Quantum (requires Python + qiskit)");
    println!("                             - google   : Google Cirq (requires Python + cirq)");
    println!("                             - aws      : AWS Braket");
    println!("                             - ionq     : IonQ");
    println!("    --device <name>          Specify device name");
    println!("    --shots <number>         Number of measurements (default: 1024)");
    println!("    --api-token <token>      API authentication token");
    println!("    --list-devices           List available quantum devices");
    println!("    --no-optimize            Disable circuit optimization");
    println!();
    println!("EXAMPLES:");
    println!("    # Run a Quantica program (interpreted)");
    println!("    quantica hello.qc");
    println!();
    println!("    # Compile to executable");
    println!("    quantica --compile app.qc");
    println!();
    println!("    # JIT compile and run with optimizations");
    println!("    quantica --run -O3 program.qc");
    println!();
    println!("    # Start REPL");
    println!("    quantica --repl");
    println!();
    println!("    # Generate documentation");
    println!("    quantica --doc lib.qc");
    println!();
    println!("    # === QUANTUM SIMULATION EXAMPLES ===");
    println!();
    println!("    # Use native Rust simulator (recommended - fast and no dependencies!)");
    println!("    quantica --hardware native bell.qc");
    println!();
    println!("    # Native simulator with 10,000 shots");
    println!("    quantica --hardware native --shots 10000 grover.qc");
    println!();
    println!("    # Use IBM Qiskit backend (requires Python)");
    println!("    quantica --hardware ibm --device ibmq_lima bell.qc");
    println!();
    println!("    # Use Google Cirq backend (requires Python)");
    println!("    quantica --hardware google bell.qc");
    println!();
    println!("    # List available devices for a provider");
    println!("    quantica --hardware native --list-devices");
    println!();
    println!("BACKEND COMPARISON:");
    println!("    Native:  ✅ Fast, no dependencies, works offline");
    println!("    Qiskit:  ✅ Real IBM hardware, ❌ requires Python");
    println!("    Cirq:    ✅ Google integration, ❌ requires Python");
    println!();
    println!(
        "For more information, visit: https://github.com/Quantica-Foundation/quantica-lang.git"
    );
}
fn compile_file_llvm(
    filename: &str,
    output_file: &str,
    emit_llvm: bool,
    opt_level: OptimizationLevel,
    enable_lto: bool,
    target: CompilationTarget,
) -> Result<(), String> {
    println!("📄 Compiling: {}\n", filename);

    // Read source
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;

    // Lexical Analysis
    println!("🔤 Phase 1: Lexical Analysis");
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;

    //Syntax Analysis (Parsing)
    println!("🌳 Phase 2: Syntax Analysis");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().map_err(|e| format!("Parser error: {}", e))?;

    //Type Checking
    println!("🔬 Phase 3: Type Checking");
    TypeChecker::check_program(&ast).map_err(|e| format!("Type error: {}", e))?;
    println!("✓ Type check succeeded!\n");

    //Initialize LLVM
    println!("🤖 Phase 4: Code Generation (LLVM)");
    let context = Context::create();
    let mut compiler = Compiler::new(&context, opt_level);

    //Compile Program
    compiler.compile_program(&ast)?;
    println!("   -> Generated LLVM IR");

    //Optimize Module
    compiler.optimize_module(opt_level)?;
    println!("   -> Optimized module");

    compiler.finalize_debug_info();
    println!("   -> Finalized debug info");

    let module_name = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("quantica_program");

    // Simulate exporting the graph for the TPU
    let hlo_ir = compiler.export_to_hlo_ir(module_name)?;
    println!("   -> XLA/HLO IR Exported");

    // LLVM IR Dump
    if emit_llvm {
        println!("\n--- GENERATED LLVM IR (Optimized) ---");
        compiler.dump_ir();
        println!("-------------------------------------\n");

        // dump the specialized HLO IR when the flag is present
        println!("--- XLA/HLO IR (TPU Target) ---");
        println!("{}", hlo_ir);
        println!("-------------------------------\n");
    }

    if target != CompilationTarget::HostCPU {
        // GPU/TPU compilation path: generate the IR and skip the linking to a .exe
        println!(
            "⚠️ Target is {:?}. Skipping AOT linking and outputting IR only.",
            target
        );

        // Simulating the final HLO/SPIR-V export by calling the exporter
        let module_name = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("quantica_program");
        let hlo_ir = compiler.export_to_hlo_ir(module_name)?;

        if emit_llvm {
            println!("--- XLA/HLO IR (TPU Target) ---");
            println!("{}", hlo_ir);
            println!("-------------------------------\n");
        }
        return Ok(()); //
    }

    // Save to object file
    compiler.write_to_object_file(output_file)?;
    println!("   -> Emitted object file");

    Ok(())
}

fn run_jit_file(
    filename: &str,
    emit_llvm: bool,
    opt_level: OptimizationLevel,
    target: CompilationTarget,
) -> Result<(), String> {
    println!("🚀 JIT Compiling and Running: {}\n", filename);

    // Read, Lex, Parse
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;

    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;

    let mut parser = Parser::new(tokens);
    let ast = parser.parse().map_err(|e| format!("Parser error: {}", e))?;

    //  Type Check
    TypeChecker::check_program(&ast).map_err(|e| format!("Type error: {}", e))?;

    if target == CompilationTarget::SPIRV {
        println!("🚀 Accelerating with SPIR-V/GPU Backend...");
    } else if target == CompilationTarget::XLA {
        println!("🚀 Accelerating with XLA/TPU Backend...");
    }

    println!("⚙️ Phase 3: Compiling for JIT...");

    let jit_program = if let ASTNode::Program(ref nodes) = ast {
        let has_main = nodes.iter().any(
            |node| matches!(node, ASTNode::FunctionDeclaration { name, .. } if name == "main"),
        );

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
    let mut compiler = Compiler::new(&context, opt_level);

    let function_names = if let ASTNode::Program(s) = &jit_program {
        s.iter()
            .filter_map(|stmt| {
                if let ASTNode::FunctionDeclaration { name, .. } = stmt {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
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
    println!();

    use std::io::Write;
    let _ = std::io::stdout().flush();

    println!("[DEBUG] About to execute JIT code...");
    let _ = std::io::stdout().flush();

    let start_time = Instant::now();
    compiler.run_jit()?;
    let duration = start_time.elapsed();

    // Flush stdout AFTER JIT execution
    let _ = std::io::stdout().flush();

    println!();
    println!(
        "⏱️  JIT Execution Time: {:.6} seconds",
        duration.as_secs_f64()
    );

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

                            // Run the test file and capture its result
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
    // 1. Read source
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;

    // 2. Lexical Analysis
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;

    // 3. Syntax Analysis (Parsing)
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().map_err(|e| format!("Parser error: {}", e))?;

    // 4. Type Checking
    TypeChecker::check_program(&ast).map_err(|e| format!("Type error: {}", e))?;

    // 5. Interpretation (Hybrid Engine)
    let env = std::rc::Rc::new(std::cell::RefCell::new(Environment::new()));
    Evaluator::register_builtins(&env);

    let mut lifecycle = QubitLifecycleManager::new(true);
    let total_qubits = count_total_qubits(&ast);
    let mut simulator = NativeSimulator::new(total_qubits);

    Evaluator::evaluate_program_with_lifecycle(&ast, &env, &mut lifecycle, &mut simulator)
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
        ASTNode::QuantumDeclaration {
            name,
            size,
            initial_state,
        } => {
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

        ASTNode::FunctionDeclaration {
            name,
            parameters,
            return_type,
            body,
            ..
        } => {
            let ret_str = if let Some(t) = return_type {
                format!(" -> {:?}", t)
            } else {
                String::new()
            };
            println!(
                "{}Function {}({} params){}",
                prefix,
                name,
                parameters.len(),
                ret_str
            );
            print_ast(body, indent + 1);
        }
        ASTNode::CircuitDeclaration {
            name,
            parameters,
            return_type,
            body,
            ..
        } => {
            let ret_str = if let Some(t) = return_type {
                format!(" -> {:?}", t)
            } else {
                String::new()
            };
            println!(
                "{}Circuit {}({} params){}",
                prefix,
                name,
                parameters.len(),
                ret_str
            );
            print_ast(body, indent + 1);
        }
        ASTNode::Return(value) => {
            println!("{}Return:", prefix);
            if let Some(v) = value {
                print_ast(v, indent + 1);
            }
        }
        ASTNode::If {
            condition,
            then_block,
            elif_blocks,
            else_block,
        } => {
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
        ASTNode::For {
            variable,
            iterator,
            body,
        } => {
            println!("{}For {} in:", prefix, variable);
            print_ast(iterator, indent + 1);
            print_ast(body, indent + 1);
        }
        ASTNode::While { condition, body } => {
            println!("{}While:", prefix);
            print_ast(condition, indent + 1);
            print_ast(body, indent + 1);
        }
        ASTNode::Binary {
            operator,
            left,
            right,
            ..
        } => {
            println!("{}Binary {:?}:", prefix, operator);
            print_ast(left, indent + 1);
            print_ast(right, indent + 1);
        }

        ASTNode::ParameterizedGate {
            ref name,
            ref parameters,
            loc: _,
            ..
        } => {
            println!(
                "{}Parameterized gate: {} with params {:?}",
                prefix, name, parameters
            );
        }
        ASTNode::Unary { operator, operand } => {
            println!("{}Unary {:?}:", prefix, operator);
            print_ast(operand, indent + 1);
        }
        ASTNode::FunctionCall {
            callee, arguments, ..
        } => {
            println!("{}FunctionCall({} args)", prefix, arguments.len());
            println!("{}  Callee:", prefix);
            print_ast(callee, indent + 2);
            println!("{}  Arguments:", prefix);
            for arg in arguments {
                print_ast(arg, indent + 2);
            }
        }
        ASTNode::Apply {
            gate_expr,
            arguments,
            ..
        } => {
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
        ASTNode::ClassDeclaration {
            name,
            superclass,
            fields,
            methods,
            constructor,
            ..
        } => {
            let super_str = if let Some(s) = superclass {
                format!(" extends {}", s)
            } else {
                String::new()
            };
            println!("{}Class {}{}:", prefix, name, super_str);

            if !fields.is_empty() {
                println!("{}  Fields:", prefix);
                for field in fields {
                    println!("{}    {}: {:?}", prefix, field.name, field.field_type);
                }
            }

            if let Some(ctor) = constructor {
                println!("{}  Constructor:", prefix);
                print_ast(ctor, indent + 2);
            }

            if !methods.is_empty() {
                println!("{}  Methods:", prefix);
                for method in methods {
                    println!("{}    Method {}:", prefix, method.name);
                    print_ast(&method.body, indent + 3);
                }
            }
        }

        ASTNode::NewInstance {
            class_name,
            arguments,
            ..
        } => {
            println!("{}New Instance of {}:", prefix, class_name);
            for arg in arguments {
                print_ast(arg, indent + 1);
            }
        }

        ASTNode::MethodCall {
            object,
            method_name,
            arguments,
            ..
        } => {
            println!("{}Method Call .{}():", prefix, method_name);
            println!("{}  Object:", prefix);
            print_ast(object, indent + 2);
            println!("{}  Arguments:", prefix);
            for arg in arguments {
                print_ast(arg, indent + 2);
            }
        }

        ASTNode::SelfRef { .. } => {
            println!("{}Self", prefix);
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
        ASTNode::Range {
            start,
            end,
            inclusive,
        } => {
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
        ASTNode::TryCatch {
            try_block,
            error_variable,
            catch_block,
        } => {
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

            let important_tokens: Vec<_> = tokens
                .iter()
                .filter(|t| !matches!(t.token, Token::Newline | Token::Indent | Token::Dedent))
                .collect();

            for (i, token) in important_tokens.iter().enumerate() {
                println!(
                    "{:4}: Line {:3}, Col {:3} - {:?}",
                    i, token.line, token.column, token.token
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
    Evaluator::register_builtins(&runtime_env);

    let mut source_buffer = String::new();
    let mut is_continuation = false;

    loop {
        // 3. Set prompt and read line
        let prompt = if is_continuation { ".. " } else { ">> " };
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            println!("Error reading line.");
            return;
        }

        let trimmed_line = line.trim();

        // Handle REPL commands
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

        // Lexer
        let mut lexer = Lexer::new(&source_buffer);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(e) if e.contains("Unterminated") => {
                // Unclosed string or comment, wait for more
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

        // Parser
        let mut parser = Parser::new(tokens.clone());
        let ast = match parser.parse() {
            Ok(tree) => tree,
            Err(e)
                if e.contains("Unexpected EOF")
                    || e.contains("Expected Indent")
                    || e.contains("Dedent") =>
            {
                is_continuation = true;
                continue;
            }
            Err(e) => {
                // Real parser error, report and reset
                println!("Parser Error: {}", e);
                source_buffer.clear();
                is_continuation = false;
                continue;
            }
        };

        if let ASTNode::Program(statements) = ast {
            for stmt in statements {
                //Type Check the single statement
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
    // Read source
    println!("   -> Step 1: Reading source file...");
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Failed to read file '{}': {}", filename, e))?;

    // Lexical Analysis
    println!("   -> Step 2: Lexing tokens...");
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;

    // Syntax Analysis (Parsing)
    println!("   -> Step 3: Parsing AST...");
    let mut parser = Parser::new(tokens);
    let ast = parser
        .parse() //
        .map_err(|e| format!("Parser error: {}", e))?;

    // 4. Generate Documentation
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
        _ => Err(format!(
            "Unknown optimization flag: {}. Use -O0, -O1, -O2, or -O3.",
            arg
        )),
    }
}

fn run_on_hardware(
    filename: &str,
    config: QuantumConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Executing on Quantum Hardware: {:?}\n", config.provider);
    println!("📄 File: {}", filename);
    println!(
        "🎯 Device: {}",
        config
            .device_name
            .as_ref()
            .unwrap_or(&"default".to_string())
    );
    println!("🎲 Shots: {}\n", config.shots);

    // Read source
    let source = fs::read_to_string(filename).map_err(|e| format!("Failed to read file: {}", e))?;

    // Lex
    println!("🔤 Phase 1: Lexical Analysis");
    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexer error: {}", e))?;

    // Parse
    println!("🌳 Phase 2: Syntax Analysis");
    let mut parser = Parser::new(tokens);
    let ast = parser.parse().map_err(|e| format!("Parser error: {}", e))?;

    // Type Check
    println!("🔬 Phase 3: Type Checking");
    TypeChecker::check_program(&ast).map_err(|e| format!("Type error: {}", e))?;
    println!("✓ Type check succeeded!\n");

    // Execute on hardware
    println!("📡 Phase 4: Hardware Execution");
    let env = std::rc::Rc::new(std::cell::RefCell::new(Environment::new()));
    let mut executor = HardwareExecutor::new(config);

    let result = executor
        .execute_on_hardware(&ast, &env)
        .map_err(|e| format!("Hardware execution error: {}", e))?;

    if result.success {
        println!("\n✅ Hardware execution successful!");

        // Show top results
        if !result.counts.is_empty() {
            println!("\n📊 Measurement Results:");
            let mut sorted: Vec<_> = result.counts.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));

            for (i, (bitstring, count)) in sorted.iter().take(10).enumerate() {
                let prob = **count as f64 / result.shots as f64;
                println!(
                    "  {}. |{}⟩: {} ({:.2}%)",
                    i + 1,
                    bitstring,
                    count,
                    prob * 100.0
                );
            }
        }
    } else {
        eprintln!(
            "❌ Hardware execution failed: {}",
            result.error_message.unwrap_or("Unknown error".to_string())
        );
    }

    Ok(())
}
fn parse_error_location(error_msg: &str) -> Option<parser::ast::Loc> {
    let start_bytes = error_msg.find("[Line ")?;
    let rest = &error_msg[start_bytes..];
    let end_bytes = rest.find(']')?;
    let content = &rest[6..end_bytes];

    let parts: Vec<&str> = content.split(", Col ").collect();
    if parts.len() == 2 {
        let line = parts[0].parse().ok()?;
        let column = parts[1].parse().ok()?;
        return Some(parser::ast::Loc { line, column });
    }
    None
}
fn count_total_qubits(node: &ASTNode) -> usize {
    match node {
        ASTNode::Program(stmts) | ASTNode::Block(stmts) => {
            stmts.iter().map(count_total_qubits).sum()
        }
        ASTNode::QuantumDeclaration { size, .. } => {
            if let Some(size_expr) = size {
                if let ASTNode::IntLiteral(n) = &**size_expr {
                    *n as usize
                } else {
                    1
                }
            } else {
                1
            }
        }
        ASTNode::If {
            then_block,
            elif_blocks,
            else_block,
            ..
        } => {
            let mut sum = count_total_qubits(then_block);
            for (_, block) in elif_blocks {
                sum += count_total_qubits(block);
            }
            if let Some(blk) = else_block {
                sum += count_total_qubits(blk);
            }
            sum
        }
        ASTNode::For { body, .. } | ASTNode::While { body, .. } => count_total_qubits(body),

        _ => 0,
    }
}

