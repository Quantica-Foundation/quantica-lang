// tests/stdlib_math.qc
print("--- Running Standard Library (Math) Tests ---")

import "stdlib/math.qc" as math

let pi_val = math.PI
let abs_val = math.abs(-10.5)

assert(pi_val > 3.14, "math.PI constant import")
assert(abs_val == 10.5, "math.abs() function import")

print("--- Standard Library (Math) Tests Passed! ---")