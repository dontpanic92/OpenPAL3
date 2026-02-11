# 仙三开源版

[![Windows](https://img.shields.io/github/actions/workflow/status/dontpanic92/OpenPAL3/ci-windows.yml?branch=master&style=flat-square&label=Windows&logo=windows)](https://github.com/dontpanic92/OpenPAL3/actions/workflows/ci-windows.yml?query=branch%3Amaster)
[![Linux](https://img.shields.io/github/actions/workflow/status/dontpanic92/OpenPAL3/ci-linux.yml?branch=master&style=flat-square&label=Linux&logo=linux)](https://github.com/dontpanic92/OpenPAL3/actions/workflows/ci-linux.yml?query=branch%3Amaster)
[![macOS](https://img.shields.io/github/actions/workflow/status/dontpanic92/OpenPAL3/ci-macos.yml?branch=master&style=flat-square&label=macOS&logo=apple)](https://github.com/dontpanic92/OpenPAL3/actions/workflows/ci-macos.yml?query=branch%3Amaster)
[![Android](https://img.shields.io/github/actions/workflow/status/dontpanic92/OpenPAL3/ci-android.yml?branch=master&style=flat-square&label=Android&logo=android)](https://github.com/dontpanic92/OpenPAL3/actions/workflows/ci-android.yml?query=branch%3Amaster)
[![PSVita](https://img.shields.io/github/actions/workflow/status/dontpanic92/OpenPAL3/ci-psvita.yml?branch=master&style=flat-square&label=PlayStation%20Vita&logo=playstation)](https://github.com/dontpanic92/OpenPAL3/actions/workflows/ci-psvita.yml?query=branch%3Amaster)


> 云对雨，雪对风，仙剑对妖弓。

《仙剑奇侠传三》开源实现。仙三开源版仍处于早期开发阶段，存在很多未实现的功能及 Bug，暂无完善的游戏体验。

## 📢 声明

- 仙三开源版不附带任何《仙剑奇侠传三》的游戏数据，因此您必须拥有《仙剑奇侠传三》的正版拷贝才可以正常运行仙三开源版。
- 仙三开源版并非软星公司或大宇集团的官方作品。

## 🏡 社区

欢迎加入 QQ 群 636662894

## 📌 下载

v0.3 版本请前往 Releases 页面下载，[Azure Pipelines](https://dontpanic92.visualstudio.com/OpenPAL3/_build?definitionId=5&_a=summary&repositoryFilter=5&branchFilter=9) 上可以获得最新开发版本。

### 运行

**首次运行前请手动修改 `openpal3.toml`，将《仙剑奇侠传三》游戏目录填入：**
**（请注意反斜杠需要重复两次）**

```
# PAL3.exe 所在的目录
# The folder where PAL3.exe is
asset_path = "E:\\CubeLibrary\\apps\\1000039"
```

之后运行 `openpal3.exe` 即可。如果运行时提示 OpenAL 出错，[请下载并安装 OpenAL](http://www.openal.org/downloads/oalinst.zip)。

### 操作

- 空格键：对话框下一句
- A/D键：调整视角
- Esc键：跳过过场动画
- F键：互动
- 方向键：跑
- 1/2/3/4：存档至第1、2、3、4号存档位

## 🛠 本地构建

详细构建步骤请参阅 [BUILD_INSTRUCTIONS.md](docs/BUILD_INSTRUCTIONS.md)。

## 🙋‍♂️ 贡献

非常感谢一同参与 OpenPAL3 的开发！请参看 [CONTRIBUTING](CONTRIBUTING.md) 来了解参与项目的要求与步骤。

## 📔 相关资料

- [@zhangboyang/PAL3patch](https://github.com/zhangboyang/PAL3patch) 提供了简单有效的脱壳工具
- [仙剑三高难度吧](https://tieba.baidu.com/f?kw=%E4%BB%99%E5%89%913%E9%AB%98%E9%9A%BE%E5%BA%A6) 有一些关于仙剑三数据文件的目录结构与内容的讨论
- [这个转帖](https://tieba.baidu.com/p/5381666939?red_tag=0041464978) 分析了 `pol` 文件的部分结构
- [看雪论坛的这篇帖子](https://bbs.pediy.com/thread-157228.htm) 分析了 `cpk` 与 `sce` 的文件格式
