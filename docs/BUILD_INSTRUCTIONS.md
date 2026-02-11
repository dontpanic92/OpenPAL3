# 构建指南

本文档介绍如何在 Windows、Linux、macOS 以及 PlayStation Vita 平台上构建本项目。

## 通用前置条件

1.  **Rust Toolchain**: 
    - Windows/Linux/macOS 安装 Stable 版本即可。
    - PlayStation Vita 开发需要 **Nightly** 版本。
    - 请访问 [rustup.rs](https://rustup.rs/) 获取安装方法。

2.  **Git**: 
    - 克隆项目后请初始化子模块：
      ```bash
      git clone --recursive https://github.com/OpenPAL3/OpenPAL3.git
      ```
    - 如果已经克隆了项目，可以使用以下命令更新子模块：
      ```bash
      git submodule update --init --recursive
      ```

3.  **vcpkg**:
    - 本项目使用 `vcpkg` 管理 ffmpeg 依赖。
    - 请参考 [vcpkg 官方文档](https://github.com/microsoft/vcpkg) 安装 vcpkg，并确保将其添加到系统环境变量中，或者在构建时指定路径。

---

## Windows 构建

### 1. 安装依赖

*   **Vulkan SDK**:
    *   下载并安装最新的 [LunarG Vulkan SDK](https://vulkan.lunarg.com/)

*   **安装 ffmpeg**:
    使用 PowerShell 在仓库根目录运行以下命令安装依赖：
    ```powershell
    vcpkg install --triplet=x64-windows-static-md
    ```

    编译好的 ffmpeg 二进制文件将自动保存在 `vcpkg_installed` 目录下。

### 2. 构建项目

```powershell
# 如果 Vulkan SDK 未自动添加到环境变量，可能需要手动添加，例如：
# $env:Path += ";C:\VulkanSDK\1.4.304.0\Bin"

cargo build --workspace --release
```

构建完成后，可执行文件 `yaobow.exe` 和 `yaobow_editor.exe` 将位于 `target/release/` 目录下。

### 3. 运行

运行前，请将项目根目录下的 `openpal3.toml` 配置文件复制到可执行文件同级目录。

---

## Linux 构建

以下步骤基于 Ubuntu 22.04 测试，其他 Linux 发行版应当也可以构建成功，可能需要适当替换依赖的安装命令。

### 1. 安装依赖

- 构建时需要依赖 `nasm` 和 `libudev-dev`，可使用系统包管理器进行安装。 
- Ubuntu 22.04 官方仓库没有提供 `vulkan-sdk` 包，需要添加 LunarG 的第三方软件源。
  - 新版本 Ubuntu 和其他 Linux 发行版的官方仓库中可能已经包含了 Vulkan SDK，可以直接安装。

```bash
# Ubuntu 2204: 导入 LunarG 签名及其仓库 (Vulkan SDK)
wget -qO- https://packages.lunarg.com/lunarg-signing-key-pub.asc | sudo gpg --dearmor -o /etc/apt/trusted.gpg.d/lunarg.gpg
sudo wget -qO /etc/apt/sources.list.d/lunarg-vulkan-jammy.list https://packages.lunarg.com/vulkan/lunarg-vulkan-jammy.list

sudo apt update
sudo apt install nasm vulkan-sdk libudev-dev
```

### 2. 安装 ffmpeg

```bash
vcpkg install --triplet=x64-linux
```

### 3. 构建

```bash
cargo build --workspace --release
```

### 4. 打包 (AppImage) - 可选

如果需要生成 AppImage，可以使用 `linuxdeploy`：

```bash
mkdir -p target/AppDir
cd target
wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

# 准备 AppDir
./linuxdeploy-x86_64.AppImage --appdir AppDir
cp ../packaging/AppImage/* ./AppDir/
cp ./release/yaobow ./AppDir/usr/bin/

# 生成 AppImage
./linuxdeploy-x86_64.AppImage --appdir AppDir --output appimage
```

---

## macOS 构建

### 1. 安装依赖

推荐使用 Homebrew 安装系统依赖：

```bash
brew install nasm molten-vk vulkan-headers vulkan-loader shaderc
```

### 2. 安装 ffmpeg

```bash
# Apple Silicon
vcpkg install --triplet=arm64-osx

# Intel Mac
# vcpkg install --triplet=x64-osx
```

### 3. 构建

```bash
cargo build --workspace --release
```

---

## PlayStation Vita 构建

Vita 的构建需要依赖 `vitasdk` 和 `cargo-make`。建议在 Linux 环境下进行。以下步骤基于 Ubuntu 22.04 版本，其他 Linux 发行版需要适当替换系统命令。

### 1. 准备环境

*   安装系统工具：
    ```bash
    sudo apt install libarchive-tools libudev-dev
    ```

*   安装 Rust Nightly 和 `rust-src` 组件：
    ```bash
    rustup toolchain install nightly
    rustup component add rust-src --toolchain nightly
    ```

*   安装 `cargo-make`：
    ```bash
    cargo install cargo-make
    ```

### 2. 安装 VitaSDK

```bash
git clone https://github.com/vitasdk/vdpm
cd vdpm
./bootstrap-vitasdk.sh
./install-all.sh
```

请确保设置了以下环境变量（建议写入 shell 配置文件）：
```bash
# 请将 /usr/local/vitasdk 替换为 vitasdk 的真实路径
export VITASDK=/usr/local/vitasdk
export PATH=$VITASDK/bin:$PATH
```

### 3. 为 Vita 安装 ffmpeg

```bash
cd yaobow/misc/
vita-makepkg
vdpm ffmpeg-7.1.1-1-arm.tar.xz
```

### 4. 构建 VPK

```bash
cd yaobow/yaobow
cargo make vpk
```

构建成功后，`yaobow.vpk` 将位于 `target/armv7-sony-vita-newlibeabihf/vita-release/` 目录下。
