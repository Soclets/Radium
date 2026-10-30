# Comments and tracing

From D18. These are the implementation's comments (Rust and C++). Comments in Radium code are D86 and D87.

- **Section markers** mark each part, so scripts can find it. They mirror Radium's blocks (D13):
  ```rust
  // section prover.layer2.loops:
  …
  // end section prover.layer2.loops
  ```
- **Trace tags** link code and tests to the decision they carry out:
  ```rust
  // trace D7: forever loops must prove each trip ends
  ```
- **A checker script** in the test suite fails the build on an unclosed or misnamed section, or a tag naming a
  decision that doesn't exist. It reports every decision with no implementation or no test as a gap.
