// src/hardware_integration.rs

use crate::quantum_backend::{HardwareCircuit, HardwareGate, QuantumConfig, BackendManager, QuantumResult};
use crate::parser::ast::ASTNode;
use crate::environment::{Environment, RuntimeValue};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

/// Circuit Recorder - captures quantum operations for hardware execution
pub struct CircuitRecorder {
    num_qubits: usize,
    gates: Vec<HardwareGate>,
    measurements: Vec<usize>,
    qubit_mapping: HashMap<String, usize>, // Variable name -> qubit index
}

impl CircuitRecorder {
    pub fn new() -> Self {
        CircuitRecorder {
            num_qubits: 0,
            gates: Vec::new(),
            measurements: Vec::new(),
            qubit_mapping: HashMap::new(),
        }
    }
    
    /// Register a quantum register
    pub fn register_qubits(&mut self, var_name: &str, size: usize) -> usize {
        let start_idx = self.num_qubits;
        self.qubit_mapping.insert(var_name.to_string(), start_idx);
        self.num_qubits += size;
        start_idx
    }
    
    /// Record a gate operation
    pub fn record_gate(&mut self, gate_name: &str, qubits: Vec<usize>, params: Vec<f64>, is_dagger: bool) {
        self.gates.push(HardwareGate {
            name: gate_name.to_string(),
            qubits,
            params,
            is_dagger,
        });
    }
    
    /// Record a measurement
    pub fn record_measurement(&mut self, qubit: usize) {
        if !self.measurements.contains(&qubit) {
            self.measurements.push(qubit);
        }
    }
    
    /// Build the final hardware circuit
    pub fn build_circuit(&self) -> HardwareCircuit {
        HardwareCircuit {
            num_qubits: self.num_qubits,
            gates: self.gates.clone(),
            measurements: self.measurements.clone(),
        }
    }
}

/// Hardware Execution Mode
pub struct HardwareExecutor {
    recorder: CircuitRecorder,
    config: QuantumConfig,
}

impl HardwareExecutor {
    pub fn new(config: QuantumConfig) -> Self {
        HardwareExecutor {
            recorder: CircuitRecorder::new(),
            config,
        }
    }
    
    /// Execute a Quantica program on real hardware
    pub fn execute_on_hardware(&mut self, program: &ASTNode, env: &Rc<RefCell<Environment>>) -> Result<QuantumResult, String> {
        // Step 1: Record all quantum operations
        self.record_program(program, env)?;
        
        // Step 2: Build the circuit
        let circuit = self.recorder.build_circuit();
        
        println!("📡 Submitting to {:?} with {} qubits and {} gates", 
                 self.config.provider, circuit.num_qubits, circuit.gates.len());
        
        // Step 3: Submit to hardware backend
        let backend_manager = BackendManager::new(self.config.clone());
        let result = backend_manager.execute_circuit(&circuit)?;
        
        // Step 4: Process results
        self.process_results(&result);
        
        Ok(result)
    }
    
    /// Record quantum operations from AST
    fn record_program(&mut self, node: &ASTNode, env: &Rc<RefCell<Environment>>) -> Result<(), String> {
        match node {
            ASTNode::Program(statements) => {
                for stmt in statements {
                    self.record_statement(stmt, env)?;
                }
                Ok(())
            }
            _ => Err("Expected Program node".to_string())
        }
    }
    
    fn record_statement(&mut self, node: &ASTNode, env: &Rc<RefCell<Environment>>) -> Result<(), String> {
        match node {
            ASTNode::QuantumDeclaration { name, size, .. } => {
                let qsize = if let Some(size_expr) = size {
                    if let ASTNode::IntLiteral(n) = &**size_expr {
                        *n as usize
                    } else {
                        return Err("Quantum register size must be a literal".to_string());
                    }
                } else {
                    1
                };
                
                self.recorder.register_qubits(name, qsize);
                Ok(())
            }
            
            ASTNode::Apply { gate_expr, arguments, .. } => {
                self.record_gate_application(gate_expr, arguments)?;
                Ok(())
            }
            
            ASTNode::Measure(qubit_expr) => {
                let qubit_idx = self.extract_qubit_index(qubit_expr)?;
                self.recorder.record_measurement(qubit_idx);
                Ok(())
            }
            
            ASTNode::Block(statements) => {
                for stmt in statements {
                    self.record_statement(stmt, env)?;
                }
                Ok(())
            }
            
            ASTNode::FunctionDeclaration { body, .. } | ASTNode::CircuitDeclaration { body, .. } => {
                self.record_statement(body, env)
            }
            
            _ => Ok(()) // Skip non-quantum statements
        }
    }
    
    fn record_gate_application(&mut self, gate_expr: &ASTNode, arguments: &[ASTNode]) -> Result<(), String> {
        let (gate_name, params, is_dagger) = self.parse_gate_expression(gate_expr)?;
        let qubit_indices = self.extract_qubit_indices(arguments)?;
        
        self.recorder.record_gate(&gate_name, qubit_indices, params, is_dagger);
        Ok(())
    }
    
    fn parse_gate_expression(&self, node: &ASTNode) -> Result<(String, Vec<f64>, bool), String> {
        match node {
            ASTNode::Gate { name, .. } => Ok((name.to_lowercase(), vec![], false)),
            
            ASTNode::ParameterizedGate { name, parameters, .. } => {
                let mut params = vec![];
                for param_node in parameters {
                    if let ASTNode::FloatLiteral(f) = param_node {
                        params.push(*f);
                    } else if let ASTNode::IntLiteral(i) = param_node {
                        params.push(*i as f64);
                    } else {
                        return Err("Gate parameters must be numeric literals".to_string());
                    }
                }
                Ok((name.to_lowercase(), params, false))
            }
            
            ASTNode::Dagger { gate_expr, .. } => {
                let (name, params, _) = self.parse_gate_expression(gate_expr)?;
                Ok((name, params, true))
            }
            
            ASTNode::Controlled { gate_expr, .. } => {
                let (name, params, is_dagger) = self.parse_gate_expression(gate_expr)?;
                Ok((format!("c{}", name), params, is_dagger))
            }
            
            _ => Err("Invalid gate expression".to_string())
        }
    }
    
    fn extract_qubit_indices(&self, arguments: &[ASTNode]) -> Result<Vec<usize>, String> {
        let mut indices = vec![];
        for arg in arguments {
            indices.push(self.extract_qubit_index(arg)?);
        }
        Ok(indices)
    }
    
    fn extract_qubit_index(&self, node: &ASTNode) -> Result<usize, String> {
        match node {
            ASTNode::ArrayAccess { array, index, .. } => {
                let var_name = if let ASTNode::Identifier { name, .. } = &**array {
                    name
                } else {
                    return Err("Invalid qubit access".to_string());
                };
                
                let base_idx = self.recorder.qubit_mapping.get(var_name)
                    .ok_or("Unknown quantum register")?;
                
                let offset = if let ASTNode::IntLiteral(i) = &**index {
                    *i as usize
                } else {
                    return Err("Qubit index must be a literal".to_string());
                };
                
                Ok(base_idx + offset)
            }
            _ => Err("Invalid qubit expression".to_string())
        }
    }
    
    fn process_results(&self, result: &QuantumResult) {
        println!("\n📊 Quantum Hardware Results:");
        println!("   Shots: {}", result.shots);
        
        let mut sorted_counts: Vec<_> = result.counts.iter().collect();
        sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
        
        for (bitstring, count) in sorted_counts.iter().take(10) {
            let probability = **count as f64 / result.shots as f64;
            println!("   |{}⟩: {} ({:.2}%)", bitstring, count, probability * 100.0);
        }
    }
}

/// CLI Configuration for hardware execution
pub fn parse_hardware_config(args: &[String]) -> Option<QuantumConfig> {
    let mut config = QuantumConfig::default();
    let mut i = 0;
    
    while i < args.len() {
        match args[i].as_str() {
            "--hardware" => {
                if i + 1 < args.len() {
                    config.provider = match args[i + 1].as_str() {
                        "ibm" => crate::quantum_backend::QuantumProvider::IBM,
                        "aws" => crate::quantum_backend::QuantumProvider::AWS,
                        "ionq" => crate::quantum_backend::QuantumProvider::IonQ,
                        "google" => crate::quantum_backend::QuantumProvider::GoogleCircuit,
                        _ => return None,
                    };
                    i += 2;
                } else {
                    return None;
                }
            }
            "--device" => {
                if i + 1 < args.len() {
                    config.device_name = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return None;
                }
            }
            "--shots" => {
                if i + 1 < args.len() {
                    if let Ok(shots) = args[i + 1].parse::<u32>() {
                        config.shots = shots;
                    }
                    i += 2;
                } else {
                    return None;
                }
            }
            "--api-token" => {
                if i + 1 < args.len() {
                    config.api_token = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return None;
                }
            }
            _ => i += 1,
        }
    }
    
    Some(config)
}