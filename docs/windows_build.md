# Building OpenPAL3 on Windows

### Setting up the build environment

You will need to download and install the following tools:

Required Softwares:

- Visual Studio Build Tools
    - With the "Visual C++ Build Tools" workload
    - MSVC v143 or newer recommended
- Rust
    - Install using rustup
    - Switch to nightly: rustup default nightly
- Vulkan SDK
    - Download and install from LunarG
    - Latest version recommended (tested with 1.4.304.0)
- OpenAL
    - Download and install OpenAL 1.1 Core PC SDK
- vcpkg
    - Set up vcpkg following the official guide

### Building OpenPAL3

1. Clone the repository:

```bash
git clone https://github.com/dontpanic92/OpenPAL3.git
cd OpenPAL3
```

2. Initialize submodules:

```bash
git submodule update --init
```

3. Install dependencies via vcpkg:

```bash
vcpkg install --triplet x64-windows-static-md
```

4. Build the project:

```bash
$env:RUSTFLAGS="-A explicit_builtin_cfgs_in_flags"
cargo build --release
```

### Notes

- The RUSTFLAGS environment variable is required to prevent compilation errors related to the `--cfg windows` flag
- The build output will be located in target/release/
- Make sure all environment variables are properly set up before building

### Known Issues
- If you encounter a --cfg windows compilation error, ensure you've set the RUSTFLAGS environment variable as shown above
- If cargo can't find MSVC, ensure Visual Studio Build Tools is properly installed and the environment is configured
