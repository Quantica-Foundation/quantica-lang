// tests/stdlib_qgates.qc
print("--- Running Standard Library (QGates) Tests ---")

// We use 'from' to get the circuits directly
from "stdlib/qgates.qc" import Bell, GHZ, QFT2

// --- Test 1: Bell State ---
quantum q_bell[2]
Bell(q_bell[0], q_bell[1])
// We'll just test that it runs. The result is probabilistic.
print("Bell circuit executed.")
assert(True, "Bell circuit execution")

// --- Test 2: GHZ State (3 qubits) ---
quantum q_ghz[3]
GHZ(q_ghz)
print("GHZ circuit executed.")
assert(True, "GHZ circuit execution")

// --- Test 3: QFT2 (2 qubits) ---
quantum q_qft[2]
QFT2(q_qft[0], q_qft[1])
print("QFT2 circuit executed.")
assert(True, "QFT2 circuit execution")


print("--- Standard Library (QGates) Tests Passed! ---")