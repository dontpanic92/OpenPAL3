# 仙三开源版

[![Build Status](https://dontpanic92.visualstudio.com/OpenPAL3/_apis/build/status/dontpanic92.OpenPAL3?branchName=master)](https://dontpanic92.visualstudio.com/OpenPAL3/_build/latest?definitionId=5&branchName=master)

（刚刚开工的）《仙剑奇侠传三》开源实现。

## 📢 声明

- 仙三开源版不附带任何《仙剑奇侠传三》的游戏数据，因此您必须拥有《仙剑奇侠传三》的正版拷贝才可以正常运行仙三开源版。
- 仙三开源版并非软星公司或大宇集团的官方作品。

## 📌 项目状态

仙三开源版仍处于早期开发阶段，目前尚无可体验的游戏内容。v0.1 版本的游戏展示可以参看 [知乎文章](https://zhuanlan.zhihu.com/p/122532099) 中附带的视频。

[Azure Pipelines](https://dontpanic92.visualstudio.com/OpenPAL3/_build?definitionId=5&_a=summary&repositoryFilter=5&branchFilter=9) 上可以获得预编译程序，但仍需要一定的手工操作才可以运行，并极有可能遇到 Bug，因此不推荐普通用户下载尝试。

## 🏡 社区

欢迎加入企鹅群 636662894🎉

## 🛠 本地构建

目前 OpenPAL3 仅支持 Windows 作为目标平台，未来会对其他操作系统提供支持。

### 工具链与依赖库

在构建 OpenPAL3 前，请确认已安装以下工具链与依赖库：

- [Rust](https://www.rust-lang.org/) nightly toolchain
  - 理论上 MSVC ABI 工具链与 GNU ABI 工具链均可编译
- [.NET Core SDK 3](https://dotnet.microsoft.com/download) 或 [Visual Studio](https://visualstudio.microsoft.com/zh-hans/)
- [OpenAL](https://www.openal.org)
- [最新的 Vulkan SDK](https://www.lunarg.com/vulkan-sdk/)

### 构建步骤

1. 构建 OpenGB
    ```
    cd opengb && cargo build --release
    ```

1. 构建 OpenPAL3
   ```
   cd openpal3
   dotnet build -c Release
   dotnet publish -c Release
   ```

### 运行

目前 OpenPAL3 不支持直接读取 Cpk 文件，请使用 [RpgViewer](http://pigspy.ys168.com/) 将 Cpk 文件解压后再运行。

## 🙋‍♂️ 贡献

非常感谢一同参与 OpenPAL3 的开发！请参看 [CONTRIBUTING](CONTRIBUTING.md) 来了解参与项目的要求与步骤。

## 📔 相关资料

- [@zhangboyang/PAL3patch](https://github.com/zhangboyang/PAL3patch) 提供了简单有效的脱壳工具
- [仙剑三高难度吧](https://tieba.baidu.com/f?kw=%E4%BB%99%E5%89%913%E9%AB%98%E9%9A%BE%E5%BA%A6) 有一些关于仙剑三数据文件的目录结构与内容的讨论
- [这个转帖](https://tieba.baidu.com/p/5381666939?red_tag=0041464978) 分析了 `pol` 文件的部分结构
- [看雪论坛的这篇帖子](https://bbs.pediy.com/thread-157228.htm) 分析了 `cpk` 与 `sce` 的文件格式
