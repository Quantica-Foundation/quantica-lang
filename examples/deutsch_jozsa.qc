// Deutsch-Jozsa algorithm example
// Determines whether a function f:{0,1} → {0,1} is constant or balanced
// Uses 2 qubits: 1 input + 1 ancilla
// Based on bell_test.qc style

func main():

    // Create 2-qubit register
    quantum q[2]

    // Step 1: Initialize ancilla qubit to |1⟩
    apply X(q[1])

    // Step 2: Create superposition on both qubits
    apply Hadamard(q[0])
    apply Hadamard(q[1])

    // Step 3: Oracle (example implementation)
    // ───────────────────────────────
    // Constant function f(x) = 0 → do nothing
    // Constant function f(x) = 1 → apply X(q[1])
    // Balanced function f(x) = x → apply CNOT(q[0], q[1])
    //
    // Сейчас стоит constant-0 (самый простой). 
    // Просто раскомментируй нужную строку ниже для другого случая:

    //apply X(q[1])        // constant f(x)=1
    //apply CNOT(q[0], q[1]) // balanced f(x)=x

    // Step 4: Apply Hadamard to input qubit again
    apply Hadamard(q[0])

    // Step 5: Measure input qubit (ancilla ignore for result)
    let result = measure(q[0])

    print("Deutsch-Jozsa result:", result)
    print("→ 0 means function is CONSTANT")
    print("→ 1 means function is BALANCED")
    print("Deutsch-Jozsa completed successfully!")