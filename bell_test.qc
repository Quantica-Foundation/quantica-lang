// Bell State Test for IBM Quantum Hardware
func main():

    quantum q[2]
    apply Hadamard(q[0])
    apply CNOT(q[0], q[1])
    
    // Measure both qubits
    let m0 = measure(q[0])
    let m1 = measure(q[1])
    
    print("Measured qubit 0:", m0)
    print("Measured qubit 1:", m1)
    print("Bell state created successfully!")