// tests/modules_and_functions.qc
print("--- Running Module & Function Tests ---")

// --- Test 1: User-defined Function ---
func simple_add(a: Int, b: Int) -> Int:
    return a + b

let res1 = simple_add(10, 20)
assert(res1 == 30, "User-defined function call")

// --- Test 2: `import as` ---
import "tests/test_lib.qc" as lib
assert(lib.LIB_VAL == 123, "Module import 'as' (variable)")
let res2 = lib.add_five(10)
assert(res2 == 15, "Module import 'as' (function call)")

// --- Test 3: `from import` ---
from "tests/test_lib.qc" import LIB_VAL, add_five
assert(LIB_VAL == 123, "Module 'from import' (variable)")
let res3 = add_five(20)
assert(res3 == 25, "Module 'from import' (function call)")

print("--- Module & Function Tests Passed! ---")