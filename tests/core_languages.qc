// tests/core_language.qc
print("--- Running Core Language Tests ---")

// --- Variables ---
let x = 10
assert(x == 10, "let declaration (int)")
let s = "hello"
assert(s == "hello", "let declaration (string)")

// --- Assignment ---
mut y = 50
assert(y == 50, "mut declaration")
y = x + 10
assert(y == 20, "mut assignment")

// --- Operators ---
assert(2 + 2 == 4, "Addition")
assert(10 - 5 == 5, "Subtraction")
assert(4 * 3 == 12, "Multiplication")
assert(10 / 2 == 5, "Division")
assert(10 % 3 == 1, "Modulo")
assert(2 ^ 3 == 8, "Power (2^3)")
assert(2 ^ 3 ^ 2 == 512, "Power (right-associative 2^3^2)")

// --- CORRECTED LINES ---
assert(True And True, "Logic 'And'")
assert(True Or False, "Logic 'Or'")
assert(Not False, "Logic 'Not'")
// --- END CORRECTION ---

assert(10 > 5, "Comparison '>'")
assert(5 < 10, "Comparison '<'")
assert(10 >= 10, "Comparison '>='")
assert(10 <= 10, "Comparison '<='")
assert(10 != 11, "Comparison '!='")

// --- If/Else ---
mut test_val = 0
if 10 > 5:
    test_val = 1
else:
    test_val = -1
assert(test_val == 1, "If branch taken")

if 10 < 5:
    test_val = -1
elif 20 == 20:
    test_val = 2
else:
    test_val = -1
assert(test_val == 2, "Elif branch taken")

// --- While Loop ---
mut i = 0
while i < 5:
    i = i + 1
assert(i == 5, "While loop (i == 5)")

// --- For Loop (Range) ---
mut sum = 0
for i in 0..5: // 0, 1, 2, 3, 4
    sum = sum + i
assert(sum == 10, "For loop (sum 0..5)")

print("--- Core Language Tests Passed! ---")