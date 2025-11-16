// stdlib/math.qc

/// 
The Quantica Standard Math Library.
Provides common mathematical constants and functions.
///

/// 
The mathematical constant Pi (π).
///
let PI = 3.141592653589793

/// 
The mathematical constant Tau (τ), equal to 2 * PI.
///
let TAU = 6.283185307179586

/// 
The mathematical constant e, the base of the natural logarithm.
///
let E = 2.718281828459045

/// 
Returns the absolute value of a number.
///
/// 
Parameters:
- n: int or float
///
/// 
Returns:
- The absolute value of n.
///
func abs(n: Float) -> Float:
    if n < 0.0:
        return -n
    return n