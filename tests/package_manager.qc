// tests/package_manager.qc
print("--- Running Package Manager Test ---")

// This uses the new "package" import logic
import "test_pkg" as tp

assert(tp.GREETING == "Hello from a 3rd party package!", "Package variable import")
assert(tp.test_pkg_func() == 999, "Package function import")

print("--- Package Manager Test Passed! ---")