// True Quantum Random Number Generator (QRNG)
// Generates truly random bits via quantum superposition
// Based on bell_test.qc style

func main():
    // Note: print() output may not display in v0.2.1 interpreter
    print("=== Quantum True Random Number Generator ===")
    print("Generating 8 random bits (one byte)...\n")

    // mut required because we modify the array
    mut random_bits = []

    for i in 0..8:
        quantum q[1]                     // new qubit each iteration

        apply Hadamard(q[0])             // superposition |0> + |1>

        let bit = measure(q[0])          // measure returns Int

        // Add bit via concatenation (current supported method)
        random_bits = random_bits + [bit]

        print("Bit ", i, ": ", bit)

    // Convert 8 bits to number (0-255)
    mut random_byte = 0
    for i in 0..8:
        random_byte = random_byte * 2 + random_bits[i]

    print("\n🎲 Final random number (0-255): ", random_byte)
    print("This is truly random - not pseudo-random! 🚀")