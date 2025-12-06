// src/quantum_backend/mod.rs

use crate::billing::{BillingError, BillingService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
mod cirq_local;
mod ibm_qiskit;
use cirq_local::CirqLocalBackend;

/// Supported quantum hardware providers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, Hash)]
pub enum QuantumProvider {
    IBM,           // IBM Quantum (via Qiskit)
    Rigetti,       // Rigetti Computing (via pyQuil)
    IonQ,          // IonQ
    GoogleCircuit, // Google Cirq
    AWS,           // AWS Braket
    Azure,         // Azure Quantum
    Simulator,     // Local simulator (default)
}

/// Configuration for quantum hardware access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumConfig {
    pub provider: QuantumProvider,
    pub api_token: Option<String>,
    pub device_name: Option<String>,
    pub shots: u32, // Number of measurements
    pub optimize: bool,
}

impl Default for QuantumConfig {
    fn default() -> Self {
        QuantumConfig {
            provider: QuantumProvider::Simulator,
            api_token: None,
            device_name: None,
            shots: 1024,
            optimize: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareGate {
    pub name: String,
    pub qubits: Vec<usize>,
    pub params: Vec<f64>,
    pub is_dagger: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCircuit {
    pub num_qubits: usize,
    pub gates: Vec<HardwareGate>,
    pub measurements: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumResult {
    pub counts: HashMap<String, u32>,
    pub shots: u32,
    pub success: bool,
    pub error_message: Option<String>,
}

pub trait QuantumBackend {
    fn execute(
        &self,
        circuit: &HardwareCircuit,
        config: &QuantumConfig,
    ) -> Result<QuantumResult, String>;

    fn is_available(&self) -> bool;

    fn available_devices(&self) -> Vec<String>;

    fn optimize_circuit(&self, circuit: &HardwareCircuit) -> HardwareCircuit;
}

/// IBM Quantum Backend
pub struct IBMBackend {
    api_url: String,
    billing: Arc<BillingService>,
}

impl IBMBackend {
    pub fn new() -> Self {
        IBMBackend {
            api_url: "https://api.quantum-computing.ibm.com/api".to_string(),
            billing: BillingService::global(),
        }
    }

    pub fn with_billing(billing: Arc<BillingService>) -> Self {
        IBMBackend {
            api_url: "https://api.quantum-computing.ibm.com/api".to_string(),
            billing,
        }
    }

    fn to_qasm(&self, circuit: &HardwareCircuit) -> String {
        let mut qasm = String::new();
        qasm.push_str("OPENQASM 2.0;\n");
        qasm.push_str("include \"qelib1.inc\";\n");
        qasm.push_str(&format!("qreg q[{}];\n", circuit.num_qubits));
        qasm.push_str(&format!("creg c[{}];\n", circuit.measurements.len()));

        for gate in &circuit.gates {
            let gate_str = match gate.name.as_str() {
                "x" => format!("x q[{}];\n", gate.qubits[0]),
                "y" => format!("y q[{}];\n", gate.qubits[0]),
                "z" => format!("z q[{}];\n", gate.qubits[0]),
                "hadamard" | "h" => format!("h q[{}];\n", gate.qubits[0]),
                "s" => format!("s q[{}];\n", gate.qubits[0]),
                "t" => format!("t q[{}];\n", gate.qubits[0]),
                "cnot" | "cx" => format!("cx q[{}],q[{}];\n", gate.qubits[0], gate.qubits[1]),
                "cz" => format!("cz q[{}],q[{}];\n", gate.qubits[0], gate.qubits[1]),
                "swap" => format!("swap q[{}],q[{}];\n", gate.qubits[0], gate.qubits[1]),
                "rx" => format!("rx({}) q[{}];\n", gate.params[0], gate.qubits[0]),
                "ry" => format!("ry({}) q[{}];\n", gate.params[0], gate.qubits[0]),
                "rz" => format!("rz({}) q[{}];\n", gate.params[0], gate.qubits[0]),
                "cphase" => format!(
                    "cp({}) q[{}],q[{}];\n",
                    gate.params[0], gate.qubits[0], gate.qubits[1]
                ),
                _ => return format!("// Unsupported gate: {}\n", gate.name),
            };

            if gate.is_dagger {
                qasm.push_str(&format!("// Dagger of: {}", gate_str));
            } else {
                qasm.push_str(&gate_str);
            }
        }

        for (i, &qubit) in circuit.measurements.iter().enumerate() {
            qasm.push_str(&format!("measure q[{}] -> c[{}];\n", qubit, i));
        }

        qasm
    }
}

impl QuantumBackend for IBMBackend {
    fn execute(
        &self,
        circuit: &HardwareCircuit,
        config: &QuantumConfig,
    ) -> Result<QuantumResult, String> {
        let api_key = config
            .api_token
            .as_ref()
            .ok_or_else(|| "IBM backend requires a paid API key".to_string())?;

        self.billing
            .validate_api_key(api_key)
            .map_err(|err| match err {
                BillingError::Validation(msg) => msg,
                other => format!("Billing validation failed: {}", other),
            })?;

        let qasm = self.to_qasm(circuit);
        //not implemented due to paid model
        // In a real implementation(toDO):
        // Use reqwest to POST the QASM to IBM's API
        // Poll for job completion
        // Parse and return results

        // For now, return a placeholder
        Ok(QuantumResult {
            counts: HashMap::new(),
            shots: config.shots,
            success: true,
            error_message: None,
        })
    }

    fn is_available(&self) -> bool {
        true
    }

    fn available_devices(&self) -> Vec<String> {
        vec![
            "ibmq_qasm_simulator".to_string(),
            "ibmq_lima".to_string(),
            "ibmq_belem".to_string(),
            "ibmq_quito".to_string(),
        ]
    }

    fn optimize_circuit(&self, circuit: &HardwareCircuit) -> HardwareCircuit {
        circuit.clone()
    }
}

/// AWS Braket Backend
pub struct AWSBraketBackend {
    region: String,
}

impl AWSBraketBackend {
    pub fn new(region: String) -> Self {
        AWSBraketBackend { region }
    }
}

impl QuantumBackend for AWSBraketBackend {
    fn execute(
        &self,
        circuit: &HardwareCircuit,
        config: &QuantumConfig,
    ) -> Result<QuantumResult, String> {
        Err("AWS Braket backend not yet implemented".to_string())
    }

    fn is_available(&self) -> bool {
        false
    }

    fn available_devices(&self) -> Vec<String> {
        vec![
            "arn:aws:braket:::device/quantum-simulator/amazon/sv1".to_string(),
            "arn:aws:braket:us-east-1::device/qpu/ionq/Harmony".to_string(),
        ]
    }

    fn optimize_circuit(&self, circuit: &HardwareCircuit) -> HardwareCircuit {
        circuit.clone()
    }
}

pub struct BackendManager {
    backends: HashMap<QuantumProvider, Box<dyn QuantumBackend>>,
    config: QuantumConfig,
}

impl BackendManager {
    pub fn new(config: QuantumConfig) -> Self {
        let mut backends: HashMap<QuantumProvider, Box<dyn QuantumBackend>> = HashMap::new();

        backends.insert(QuantumProvider::IBM, Box::new(IBMBackend::new()));
        backends.insert(
            QuantumProvider::AWS,
            Box::new(AWSBraketBackend::new("us-east-1".to_string())),
        );
        backends.insert(
            QuantumProvider::GoogleCircuit,
            Box::new(CirqLocalBackend::new()),
        );

        BackendManager { backends, config }
    }

    pub fn execute_circuit(&self, circuit: &HardwareCircuit) -> Result<QuantumResult, String> {
        let backend = self
            .backends
            .get(&self.config.provider)
            .ok_or("Backend not available")?;

        if !backend.is_available() {
            return Err(format!(
                "Backend {:?} is not available",
                self.config.provider
            ));
        }

        let optimized_circuit = if self.config.optimize {
            backend.optimize_circuit(circuit)
        } else {
            circuit.clone()
        };

        backend.execute(&optimized_circuit, &self.config)
    }

    pub fn list_devices(&self) -> Vec<String> {
        if let Some(backend) = self.backends.get(&self.config.provider) {
            backend.available_devices()
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qasm_generation() {
        let backend = IBMBackend::new();
        let circuit = HardwareCircuit {
            num_qubits: 2,
            gates: vec![
                HardwareGate {
                    name: "hadamard".to_string(),
                    qubits: vec![0],
                    params: vec![],
                    is_dagger: false,
                },
                HardwareGate {
                    name: "cnot".to_string(),
                    qubits: vec![0, 1],
                    params: vec![],
                    is_dagger: false,
                },
            ],
            measurements: vec![0, 1],
        };

        let qasm = backend.to_qasm(&circuit);
        assert!(qasm.contains("OPENQASM 2.0"));
        assert!(qasm.contains("h q[0]"));
        assert!(qasm.contains("cx q[0],q[1]"));
    }
}
