// src/quantum_backend/ibm_qiskit.rs

use super::{HardwareCircuit, QuantumConfig, QuantumResult, QuantumBackend};
use std::collections::HashMap;
use std::process::Command;

pub struct IBMQiskitBackend;

impl IBMQiskitBackend {
    pub fn new() -> Self {
        IBMQiskitBackend
    }
    
    fn get_python_command() -> &'static str {
        if cfg!(target_os = "windows") {
            "python"
        } else {
            "python3"
        }
    }
    
    fn generate_qiskit_script(&self, circuit: &HardwareCircuit, shots: u32, device: Option<&str>) -> String {
        let mut script = String::from("from qiskit import QuantumCircuit, transpile\n");
        script.push_str("from qiskit_aer import AerSimulator\n");
        script.push_str("import json\n\n");
        
        script.push_str(&format!("qc = QuantumCircuit({}, {})\n\n", 
                                circuit.num_qubits, circuit.num_qubits));
        
        // gates
        for gate in &circuit.gates {
            let gate_code = match gate.name.as_str() {
                "hadamard" | "h" => format!("qc.h({})", gate.qubits[0]),
                "x" => format!("qc.x({})", gate.qubits[0]),
                "y" => format!("qc.y({})", gate.qubits[0]),
                "z" => format!("qc.z({})", gate.qubits[0]),
                "s" => format!("qc.s({})", gate.qubits[0]),
                "t" => format!("qc.t({})", gate.qubits[0]),
                "cnot" | "cx" => format!("qc.cx({}, {})", gate.qubits[0], gate.qubits[1]),
                "cz" => format!("qc.cz({}, {})", gate.qubits[0], gate.qubits[1]),
                "swap" => format!("qc.swap({}, {})", gate.qubits[0], gate.qubits[1]),
                "rx" => format!("qc.rx({}, {})", gate.params[0], gate.qubits[0]),
                "ry" => format!("qc.ry({}, {})", gate.params[0], gate.qubits[0]),
                "rz" => format!("qc.rz({}, {})", gate.params[0], gate.qubits[0]),
                _ => continue,
            };
            script.push_str(&format!("{}\n", gate_code));
        }
        
        // measurements
        script.push_str("\n# Measure all qubits\n");
        for i in 0..circuit.num_qubits {
            script.push_str(&format!("qc.measure({}, {})\n", i, i));
        }
        
        // Execute
        script.push_str("\n# Execute on simulator\n");
        script.push_str("simulator = AerSimulator()\n");
        script.push_str("transpiled = transpile(qc, simulator)\n");
        script.push_str(&format!("job = simulator.run(transpiled, shots={})\n", shots));
        script.push_str("result = job.result()\n");
        script.push_str("counts = result.get_counts()\n");
        script.push_str("print(json.dumps(counts))\n");
        
        script
    }
}

impl QuantumBackend for IBMQiskitBackend {
    fn execute(&self, circuit: &HardwareCircuit, config: &QuantumConfig) -> Result<QuantumResult, String> {
        let script = self.generate_qiskit_script(circuit, config.shots, config.device_name.as_deref());
        
        std::fs::write("temp_qiskit.py", &script)
            .map_err(|e| format!("Failed to write script: {}", e))?;
        
        let python_cmd = Self::get_python_command();
        let output = Command::new(python_cmd)
            .arg("temp_qiskit.py")
            .output()
            .map_err(|e| format!("Failed to execute Python (tried '{}'): {}", python_cmd, e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Python execution failed: {}", stderr));
        }
        
        let result_json = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&result_json)
            .map_err(|e| format!("Parse error: {}\nOutput: {}", e, result_json))?;
        
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
            .arg("import qiskit")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    fn available_devices(&self) -> Vec<String> {
        vec![
            "aer_simulator".to_string(),
            "ibmq_qasm_simulator".to_string(),
        ]
    }
    
    fn optimize_circuit(&self, circuit: &HardwareCircuit) -> HardwareCircuit {
        circuit.clone()
    }

}
