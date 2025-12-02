# Quantica Programming Language

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Quantica CI](https://github.com/Quantica-Foundation/quantica-lang/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Quantica-Foundation/quantica-lang/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/written%20in-Rust-orange)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Windows-lightgrey)]()
[![Website](https://img.shields.io/badge/website-live-00cc66)](https://quantica-foundation.github.io/quantica-lang/)


**Quantica** is a high-performance, compiled programming language designed for the next generation of computing: **Quantum-Classical Hybrid** workflows. Written in Rust and powered by LLVM, it features native quantum primitives, sparse state simulation, and a syntax built for scientists and engineers.

---

##  Why Quantica?

Most quantum languages are just Python libraries. Quantica is a **compiled language** where quantum gates, measurements, and circuits are first-class citizens.

* **Native Quantum Syntax:** Allocate registers (`quantum q[2]`) and apply gates (`apply Hadamard(q[0])`) directly in the language.
* **Hybrid Runtime:** Mix classical logic (`if`, `while`, `functions`) with quantum state manipulation seamlessly.
* **Gate Modifiers:** Dynamically apply `dagger` (inverse) and `controlled` modifiers to *any* gate or circuit.
* **LLVM Backend:** Compiles to highly optimized native machine code via LLVM 18 for maximum performance.
* **Package Management:** Built-in support for modules and packages via `import` and `from` syntax.

---

## 📦 Installation

### Option 1: Windows Installer (Recommended)
For the easiest setup, download the pre-built installer from our latest release.

1.  Go to the **[Releases Page](https://github.com/Quantica-Foundation/quantica-lang/releases)**.
2.  Download **`QuanticaInstaller-v0.1.0-alphatest.exe`**.
3.  Run the installer to set up Quantica and add it to your system PATH automatically.

### Option 2: Build from Source
If you are on Linux/macOS or prefer building from source, you will need [Rust](https://www.rust-lang.org/tools/install) and LLVM 18 installed.

```bash
# 1. Clone the repository
git clone [https://github.com/Quantica-Foundation/quantica-lang.git](https://github.com/Quantica-Foundation/quantica-lang.git)
cd quantica-lang

# 2. Build the compiler (release mode recommended for performance)
cargo build --release

# 3. The executable will be at ./target/release/quantica
Add this folder to your PATH or copy the binary to /usr/local/bin
```
## Known Limitations
- Match expressions incomplete (use if/elif/else)
- Try/finally not implemented (try/catch works)
- JIT mode experimental (use interpreter or AOT)

⭐ If you like this project, consider giving it a star!
