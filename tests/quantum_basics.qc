// tests/quantum_basics.qc
print("--- Running Quantum Basics Tests ---")

// --- Test 1: Declaration and X Gate ---
quantum q1[1]
apply X(q1[0])
let m1 = measure(q1[0])
assert(m1 == 1, "X gate test (measure |1>)")

// --- Test 2: Hadamard Gate ---
quantum q2[1]
apply Hadamard(q2[0])
// Measurement is probabilistic, so we can't assert a specific value
// But we can check if the test runs without errOr.
let m2 = measure(q2[0]) 
if m2 == 0:
    print("Hadamard measured 0")
else:
    print("Hadamard measured 1")
assert(m2 == 0 Or m2 == 1, "Hadamard measurement is 0 Or 1")

// --- Test 3: H H = I ---
quantum q3[1]
apply Hadamard(q3[0])
apply Hadamard(q3[0])
let m3 = measure(q3[0])
assert(m3 == 0, "Hadamard-Hadamard test (H * H = I)")

// --- Test 4: CNOT ---
quantum q4[2]
apply X(q4[0]) // State is |10>
apply CNOT(q4[0], q4[1]) // State should be |11>
let m4_0 = measure(q4[0])
let m4_1 = measure(q4[1])
assert(m4_0 == 1, "CNOT test (control bit)")
assert(m4_1 == 1, "CNOT test (target bit flipped)")

// --- Test 5: Toffoli (CCX) ---
quantum q5[3]
apply X(q5[0])
apply X(q5[1]) // State is |110>
apply controlled(controlled(X))(q5[0], q5[1], q5[2]) // State should be |111>
let m5_2 = measure(q5[2])
assert(m5_2 == 1, "Toffoli test (target bit flipped)")

// --- Test 6: Toffoli (CCX) No-op ---
quantum q6[3]
apply X(q6[0]) // State is |100>
apply controlled(controlled(X))(q6[0], q6[1], q6[2]) // State should be |100>
let m6_2 = measure(q6[2])
assert(m6_2 == 0, "Toffoli test (target bit not flipped)")


print("--- Quantum Basics Tests Passed! ---")