/*
stdlib/qgates.qc
*/

/*
Provides a standard library of common quantum circuits and algorithms,
built from the core quantum gates.
*/

// Import PI at the top level
from "stdlib/math.qc" import PI

/*
Creates the Bell state |Φ+> = (|00> + |11>)/√2.

Parameters:
  - q0: Qubit, The first qubit (control)
  - q1: Qubit, The second qubit (target)
*/
circuit Bell(q0: Qubit, q1: Qubit):
    apply Hadamard(q0)
    apply CNOT(q0, q1)

/*
Creates a GHZ (Greenberger–Horne–Zeilinger) state on N qubits.
This is the N-qubit entangled state (|00...0> + |11...1>)/√2.

Parameters:
  - qreg: QuantumRegister, The register to entangle.
*/
circuit GHZ(qreg: QuantumRegister):
    // Put the first qubit in superposition
    apply Hadamard(qreg[0])
    
    // Cascade CNOTs to entangle all other qubits
    for i in 1..qreg.length:
        apply CNOT(qreg[0], qreg[i])

/*
Performs a 2-Qubit Quantum Fourier Transform (QFT).

Parameters:
  - q0: Qubit, The first qubit
  - q1: Qubit, The second qubit
*/
circuit QFT2(q0: Qubit, q1: Qubit):
    // 1. Apply H to q0
    apply Hadamard(q0)
    
    // 2. Apply controlled RZ(PI / 2.0)(q1, q0)
    apply controlled(RZ(PI / 2.0))(q1, q0)
    
    // 3. Apply H to q1
    apply Hadamard(q1)
    
    // 4. Swap q0 and q1
    apply Swap(q0, q1)
