// tests/core.qc
print("--- Running Core Tests ---")

let x = 10
assert(x == 10, "let declaration")

mut y = 20
y = y + 5
assert(y == 25, "mut assignment")

let z = 2 ^ 3
assert(z == 8, "power operator")

print("--- Core Tests Passed! ---")