# C++

From D44. C++ is where undefined behaviour could bite, so it is fenced in.

1. **Small:** C++ only where LLVM's C API falls short, plus custom LLVM passes (D17).
2. **Modern and hardened:** C++23 (`std::span`, `std::expected`, concepts). libc++ hardening is on in every build, so
   out-of-bounds access on standard containers traps. No exceptions and no RTTI, matching LLVM's build. C++26 once
   clang supports the parts used.
3. **Restricted:** banned are raw `new`/`delete`, C-style casts, `reinterpret_cast`, unions, pointer arithmetic,
   uninitialised variables, `goto` and variadic functions. Enforced by `clang-tidy` (its C++ Core Guidelines,
   `bugprone` and `cert` checks) and compiler warnings, all as errors.
4. **Tested for undefined behaviour:** UBSan, ASan and TSan on every test run; fuzzing through the C interface.
5. **Shielded:** the wrapper exposes only small `extern "C"` functions, each with a written contract (pointer
   ownership, lifetime, allowed values). Rust wraps each once in a safe function that checks the contract; the rest of
   the compiler never touches C++ directly.
6. **A second analyser:** Coverity Scan (free for open-source projects).

**Sources that are not used:**
- MISRA's documents: their terms of sale forbid letting any AI system process them.
- The AUTOSAR C++14 guidelines: "for informational purposes only".
- The C++ Core Guidelines document may be read as a reference, but its text is never copied into the repo. (clang-tidy's
  `cppcoreguidelines` checks are part of LLVM and unaffected.)

Our rules are clang-tidy's checks plus this list, in our own words.
