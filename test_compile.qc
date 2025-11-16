// test_compile.qc
// This is the test suite wrapped in a func main() for the AOT compiler.

func main():
    let PI = 3.14159265
    
    print("=== QUANTICA COMPILER AOT TEST ===\n")
    
    // ============================================
    // 1. SINGLE-QUBIT PAULI GATES
    // ============================================
    print("--- 1. Pauli X Gate (NOT) ---")
    quantum qx[1]
    print("Initial state:")
    debug_state(qx)
    apply X(qx[0])
    print("After X gate:")
    debug_state(qx)
    
    print("\n--- 4. Hadamard Gate ---")
    quantum qh[1]
    apply Hadamard(qh[0])
    print("After H on |0>:")
    debug_state(qh)
    apply Hadamard(qh[0])
    print("After H again (should be |0>):")
    debug_state(qh)
    
    // ============================================
    // 2. ROTATION GATES
    // ============================================
    print("\n--- 7. RX Gate (Rotation around X-axis) ---")
    quantum qrx[1]
    apply RX(PI)(qrx[0])
    print("After RX(π) on |0>:")
    debug_state(qrx)
    
    // ============================================
    // 3. TWO-QUBIT GATES
    // ============================================
    print("\n--- 10. CNOT Gate (Controlled-NOT) ---")
    quantum qcnot[2]
    apply X(qcnot[0])
    print("Initial state |10>:")
    debug_state(qcnot)
    apply CNOT(qcnot[0], qcnot[1])
    print("After CNOT:")
    debug_state(qcnot)
    
    // ============================================
    // 4. CONTROLLED GATES
    // ============================================
    print("\n--- 15. Controlled-X (CNOT) via controlled() ---")
    quantum qcx[2]
    apply X(qcx[0])
    apply controlled(X)(qcx[0], qcx[1])
    print("After controlled(X):")
    debug_state(qcx)
    
    // ============================================
    // 5. DAGGER (INVERSE) GATES
    // ============================================
    print("\n--- 24. Dagger(S) = S† ---")
    quantum qds[1]
    apply X(qds[0])
    apply S(qds[0])
    print("After S:")
    debug_state(qds)
    apply dagger(S)(qds[0])
    print("After dagger(S):")
    debug_state(qds)
    
    // ============================================
    // 6. DOUBLY-CONTROLLED GATES (3-QUBIT)
    // ============================================
    print("\n--- 20. Toffoli Gate (CCX) ---")
    quantum qtof[3]
    apply X(qtof[0])
    apply X(qtof[1])
    print("Initial state |110>:")
    debug_state(qtof)
    apply controlled(controlled(X))(qtof[0], qtof[1], qtof[2])
    print("After Toffoli:")
    debug_state(qtof)
    print("\n🎉 AOT COMPILE TEST COMPLETE! 🎉")    