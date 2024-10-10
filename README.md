# kdurs: High-performance Rust bindings to Kakadu

**kdurs** is a Rust binding for the Kakadu library, a powerful and widely-used JPEG 2000 codec. **kdurs** is specifically designed for high-performance and low overhead via a carefully crafted set of optimization tricks.

## System requirements

- x64 or ARMv8 processor
- Linux kernel 5.1 or later

## Key Features

- **Asynchronous IO:** All IO work submitted during decompression is performed asynchronously IO, allowing readers and writers to overlap with decoding and maximize use of system resources.

- **Build-time optimization:** kdurs opts into all available build-time optimizations where appropriate. This results in a wide range of platform-specific build targets but significantly improves performance.

  - **LTO (Link-time optimization):** Enables whole-program optimization, spanning both Rust and C++ code, resulting in smaller executable sizes and improved runtime efficiency.
  - **PGO (Profile-guided optimization):** Analyzes code execution patterns from runtime profiling data to identify hot spots and optimize them for faster execution.
  - **Target-specific features:** Automatically selects optimizations based on the target platform's architecture and instruction set (e.g., AVX2, NEON), achieving optimal efficiency for various hardware configurations.

- **Tracing integration:** kdurs seamlessly integrates with the tracing library, enabling performance profiling and analysis, including interactions between Rust and Kakadu.

- **Minimal footprint:** Whole-program LTO and static linkage produces exceptionally compact binaries, making it suitable for resource-constrained environments such as AWS Lambda and packaging in minimal container base images.

## Getting Started

1. **Install Rust and Cargo:** Ensure that you have the Rust toolchain installed, which includes the package manager Cargo. You can download it from [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

1. **Install Clang:** Ensure you have a working Clang compiler installation. You can find instructions for your platform on the [Clang website](https://clang.llvm.org/):

   - **Linux/macOS:**
     ```bash
     sudo apt-get install clang  # Debian/Ubuntu
     sudo yum install clang     # Fedora/CentOS
     ```

1. **Install Kakadu:** Obtain and install the proprietary Kakadu library from [https://www.kakadusoftware.com/](https://www.kakadusoftware.com/). Ensure that the Kakadu library is properly configured and accessible during the build process.

   - **Note for Digirati employees**: The latest version of Kakadu is included as a submodule within this repository. To access the Kakadu source code, initialize the submodules using the following command:

     ```bash
     git submodule update --init --recursive
     ```

## Contributing

Contributions are welcome! Please feel free to open an issue or submit a pull request.
