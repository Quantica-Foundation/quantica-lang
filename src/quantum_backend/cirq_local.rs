// src/quantum_backend/cirq_local.rs

use super::{HardwareCircuit, QuantumConfig, QuantumResult, QuantumBackend};
use std::collections::HashMap;
use std::process::Command;

pub struct CirqLocalBackend;

impl CirqLocalBackend {
    pub fn new() -> Self {
        CirqLocalBackend
    }
    
    
    fn get_python_command() -> &'static str {
        if cfg!(target_os = "windows") {
            "python"
        } else {
            "python3" 
        }
    }
    
    fn generate_cirq_script(&self, circuit: &HardwareCircuit, shots: u32) -> String {
        let mut script = String::from("import cirq\nimport json\n\n");
        
        script.push_str(&format!("qubits = [cirq.LineQubit(i) for i in range({})]\n", circuit.num_qubits));
        script.push_str("circuit = cirq.Circuit()\n\n");
        
        for gate in &circuit.gates {
            let gate_code = match gate.name.as_str() {
                "hadamard" | "h" => format!("circuit.append(cirq.H(qubits[{}]))", gate.qubits[0]),
                "x" => format!("circuit.append(cirq.X(qubits[{}]))", gate.qubits[0]),
                "y" => format!("circuit.append(cirq.Y(qubits[{}]))", gate.qubits[0]),
                "z" => format!("circuit.append(cirq.Z(qubits[{}]))", gate.qubits[0]),
                "cnot" | "cx" => format!("circuit.append(cirq.CNOT(qubits[{}], qubits[{}]))", 
                                        gate.qubits[0], gate.qubits[1]),
                "rx" => format!("circuit.append(cirq.rx({}).on(qubits[{}]))", 
                               gate.params[0], gate.qubits[0]),
                "ry" => format!("circuit.append(cirq.ry({}).on(qubits[{}]))", 
                               gate.params[0], gate.qubits[0]),
                "rz" => format!("circuit.append(cirq.rz({}).on(qubits[{}]))", 
                               gate.params[0], gate.qubits[0]),
                _ => continue,
            };
            script.push_str(&format!("{}\n", gate_code));
        }
        
        script.push_str("\ncircuit.append(cirq.measure(*qubits, key='result'))\n");
        script.push_str("simulator = cirq.Simulator()\n");
        script.push_str(&format!("result = simulator.run(circuit, repetitions={})\n", shots));
        script.push_str("counts = result.histogram(key='result')\n");
        script.push_str(&format!("print(json.dumps({{format(k, '0{}b'): int(v) for k, v in counts.items()}}))\n", circuit.num_qubits));
        
        script
    }
}

impl QuantumBackend for CirqLocalBackend {
    fn execute(&self, circuit: &HardwareCircuit, config: &QuantumConfig) -> Result<QuantumResult, String> {
        let script = self.generate_cirq_script(circuit, config.shots);
        
        std::fs::write("temp_cirq.py", &script)
            .map_err(|e| format!("Failed to write script: {}", e))?;
        
        let python_cmd = Self::get_python_command();
        let output = Command::new(python_cmd)
            .arg("temp_cirq.py")
            .output()
            .map_err(|e| format!("Failed to execute Python (tried '{}'): {}", python_cmd, e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Python execution failed: {}", stderr));
        }
        
        let result_json = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&result_json)
            .map_err(|e| format!("Parse error: {}", e))?;
        
        let mut counts = HashMap::new();
        if let Some(obj) = parsed.as_object() {
            for (k, v) in obj {
                if let Some(count) = v.as_u64() {
                    counts.insert(k.clone(), count as u32);
                }
            }
        }
        
        Ok(QuantumResult {
            counts,
            shots: config.shots,
            success: true,
            error_message: None,
        })
    }
    
    fn is_available(&self) -> bool {
        let python_cmd = Self::get_python_command();
        Command::new(python_cmd)
            .arg("-c")
            .arg("import cirq")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    fn available_devices(&self) -> Vec<String> {
        vec!["local_simulator".to_string()]
    }
    
    fn optimize_circuit(&self, circuit: &HardwareCircuit) -> HardwareCircuit {
        circuit.clone()
    }

}
