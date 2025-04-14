<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

![Slint](./logo/slint-logo-full-light.svg#gh-light-mode-only) ![Slint](./logo/slint-logo-full-dark.svg#gh-dark-mode-only)

[![Build Status](https://github.com/slint-ui/slint/workflows/CI/badge.svg)](https://github.com/slint-ui/slint/actions)
[![REUSE status](https://api.reuse.software/badge/github.com/slint-ui/slint)](https://api.reuse.software/info/github.com/slint-ui/slint)
[![Discussions](https://img.shields.io/github/discussions/slint-ui/slint)](https://github.com/slint-ui/slint/discussions)

#### [English](https://github.com/slint-ui/slint/blob/master/README.md) | 简体中文

Slint 是一款声明式图形用户界面（GUI）工具包，可用于为使用 Rust、C++、JavaScript 或 Python 编写的嵌入式、桌面端和移动端应用程序构建原生用户界面。

*Slint* 这个名字源于我们的设计目标：

- **扩展性**: Slint 应支持响应式用户界面设计，允许在不同操作系统和处理器架构之间跨平台使用，并且支持多种编程语言。
- **轻量性**: 就内存和处理能力而言，Slint 应仅占用极少的资源，同时在任何设备上都能提供流畅的、类似智能手机的用户体验。
- **易用性**: 设计师和开发人员在享受图形用户界面设计和开发过程的同时，应能高效工作。对于设计师来说，设计创建工具应该易于使用。同样，对于开发人员而言，无论他们选择哪种编程语言，API 都应该具有一致性且易于使用。
- **原生性**: 使用 Slint 构建的图形用户界面应符合终端用户对于原生应用程序的期望，无论该应用程序运行于何种平台 —— 桌面端、移动端、网页端还是嵌入式系统。用户界面应编译为机器码，并提供只有原生应用程序才能具备的灵活性：能够访问完整的操作系统 API，利用所有的 CPU 和 GPU 核心，连接任何外围设备。

访问 [#MadeWithSlint](https://madewithslint.com) 查看一些使用 Slint 的项目。我们诚邀您使用 Slint 并成为其社区的一员。

## 当前状态

Slint 正在积极开发中。对每个平台的支持情况如下：

- **嵌入式**: *已就绪*。Slint 正被客户用于生产环境中的嵌入式设备的嵌入式系统如 Linux 和 Windows中。Slint 运行时所需内存不到 300KiB，并且可以在不同的处理器架构上运行，例如 ARM Cortex M、ARM Cortex A、RISC-V、Intel x86 等。
    有关支持的开发板列表，请参阅 <https://slint.dev/supported-boards>.
- **桌面端**: *开发中*。虽然 Slint 非常适用于 Windows、Linux 和 Mac 系统，但我们正在努力在后续版本中提升对这些平台的支持。
- **网页端**: *开发中*。Slint 应用程序可以编译为 WebAssembly，并能在浏览器中运行。由于存在许多其他的网页框架，浏览器平台并非我们的主要目标平台之一。目前对浏览器平台的支持仅限于演示用途。
- **移动端**
  - Android: *开发中*。可在此处跟踪工作进展：<https://github.com/slint-ui/slint/issues/46>。
  - iOS: *规划中*。在完成对安卓系统的初步支持后，将开始着手对 iOS 系统的支持工作。

### 可访问性

Slint 支持许多窗口小部件的基于键盘的导航操作，并且用户界面具有可缩放性。诸如屏幕阅读器之类的辅助技术的基本基础设施已搭建就绪。我们也意识到，要为有特殊需求的用户提供一流的支持，还需要做更多的工作。

## 演示示例

### 嵌入式

| 树莓派                               | STM32                         | RP2040                         |
| ------------------------------------ | ----------------------------- | ------------------------------ |
| [Video of Slint on Raspberry Pi][#1] | [Video of Slint on STM32][#2] | [Video of Slint on RP2040][#3] |

### 桌面端

| Windows                                     | macOS                                     | Linux                                     |
| ------------------------------------------- | ----------------------------------------- | ----------------------------------------- |
| ![Screenshot of the Gallery on Windows][#4] | ![Screenshot of the Gallery on macOS][#5] | ![Screenshot of the Gallery on Linux][#6] |

### 网页端 WASM

| 打印机示例                                     | 滑块拼图游戏                                  | 能源监测器                                            | 组件库演示                                | 天气演示                                  |
| ------------------------------------------- | -------------------------------------------- | ---------------------------------------------------- | --------------------------------------------- | --------------------------------------------- |
| [![Screenshot of the Printer Demo][#7]][#8] | [![Screenshot of the Slide Puzzle][#9]][#10] | [![Screenshot of the Energy Monitor Demo][#11]][#12] | [![Screenshot of the Gallery Demo][#13]][#14] | [![Screenshot of the weather Demo][#29]][#30] |

[示例](examples#examples) 文件夹中有更多的示例和演示内容

## 开始使用

### 你好，世界

用户界面是用一种领域特定语言来定义的，这种语言具有声明式的特点，易于使用、直观易懂，并且提供了一种强大的方式来描述图形元素、它们的位置、层级结构、属性绑定以及在不同状态下的数据流向。

下面是必不可少的 “你好，世界” 示例：

```slint
export component HelloWorld inherits Window {
    width: 400px;
    height: 400px;

    Text {
       y: parent.width / 2;
       x: parent.x + 200px;
       text: "Hello, world";
       color: blue;
    }
}
```

### 文档

如需了解更多详细信息，请查阅 [Slint 语言文档](https://slint.dev/docs/slint)。

[示例](examples) 文件夹中包含各种示例和演示，展示了如何使用 Slint 标记语言，以及如何通过受支持的编程语言与 Slint 用户界面进行交互。

`文档` 文件夹中包含更多信息，其中包括 [构建说明](docs/building.md) 以及内部人员 [开发文档](docs/development.md)。

请参考 `api` 文件夹中每种语言目录下的 README 文件：

- [C++](api/cpp) ([文档][#15] | [入门模板][#17])
- [Rust](api/rs/slint) [![Crates.io][#18]][#19] ([文档][#20] | [视频教程][#22] | [入门模板][#23])
- [JavaScript/NodeJS (测试版)](api/node) [![npm][#24]][#25] ([文档][#26] | [入门模板][#28])
- [Python (测试版)](api/python) [![pypi][#31]][#32] ([文档][#33] | [入门模板][#34])

## 架构

一个应用程序由使用 Rust、C++ 或 JavaScript 编写的业务逻辑，以及 `.slint` 用户界面设计标记组成，该标记会被编译为原生代码。

![Architecture Overview](https://slint.dev/resources/architecture.drawio.svg)

### 编译器

`.slint` 文件会被预编译。`.slint` 文件中的表达式是编译器能够优化的纯函数。例如，编译器可以选择 “内联” 属性，并去除那些常量属性或未发生变化的属性。未来，我们希望通过对`图像`和`文本`进行预处理来缩短低端设备上的渲染时间。编译器可以判定，在同一位置上的一个`文本`元素或`图像`元素，总是位于另一个`图像`元素的上方。因此，这两个元素可以提前渲染为一个单一元素，从而减少渲染时间。

编译器采用的典型编译阶段包括词法分析、语法分析、优化，最后是代码生成。它为目标语言的代码生成提供了不同的后端。C++ 代码生成器会生成一个 C++ 头文件，Rust 代码生成器会生成 Rust 代码，依此类推。它还包含一个用于动态语言的解释器。

### 运行时

运行时库包含一个引擎，该引擎支持在 `.slint` 语言中声明的属性。组件的元素、子项和属性会分布在单个内存区域中，以减少内存分配。

渲染后端和外观样式在编译时是可配置的：

- `femtovg` 渲染器使用 OpenGL ES 2.0 进行渲染。
- `skia` 渲染器使用 [Skia](https://skia.org) 进行渲染。
- `software` 软件渲染器仅使用 CPU，且没有额外的依赖项。

注意：当系统上安装了 Qt 时，就可以使用 `qt` 样式，通过 Qt 的 QStyle 来实现具有原生外观的窗口小部件。

### 工具

我们有一些工具可辅助开发 .slint 文件：

- 一个 [**语言服务器**](./tools/lsp)，它为许多编辑器添加了诸如自动补全以及 .slint 文件实时预览等功能。
- 它被捆绑在一个可从应用商店获取的 [**Visual Studio Code 扩展**](./editors/vscode) 中。
- 一个 [**slint-viewer**](./tools/viewer) 查看器工具，用于预览 .slint 文件。 使用 `--auto-reload` 参数能让你在处理用户界面时轻松预览（当无法使用 LSP 进行预览时）。
- [**SlintPad**](https://slintpad.com/), 一个无需安装任何东西即可试用.slint 语法的在线编辑器 ([源代码](./tools/slintpad))。
- 一个用于将 .slint 文件从旧版本转换为新版本的 [**更新工具**](./tools/updater)。
- 一个 [**Figma 转 Slint**](https://www.figma.com/community/plugin/1474418299182276871/figma-to-slint) 的插件。

请查看我们的编辑器 [自述](./editors/README.md) 文件，获取有关如何配置你喜欢的编辑器以使其能与 Slint 良好配合的提示。

## 许可证

你可以根据自己的选择，在以下***任何***一种许可证下使用 Slint：

1. 使用 [免费许可证](LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)，免费构建您的桌面端、移动端或网页端应用程序,
2. 使用 [GPLv3 许可证](LICENSES/GPL-3.0-only.txt)，免费构建开源的嵌入式、桌面端、移动端或网页端应用程序,
3. 使用 [付费许可证](LICENSES/LicenseRef-Slint-Software-3.0.md)，构建您的嵌入式、桌面端、移动端或网页端应用程序。

请查看我们网站上的 [Slint 许可选项](https://slint.dev/pricing.html) 以及 [许可证常见问题解答](FAQ.md#licensing)。

## 贡献

我们欢迎您以代码、错误报告或反馈等形式做出贡献。

- 如果您在某个问题上看到 [征求意见稿(RFC)](https://github.com/slint-ui/slint/labels/rfc) 标签，欢迎积极参与讨论。
- 有关贡献指南，请参阅 [CONTRIBUTING.md](CONTRIBUTING.md) 文件。

## 常见问题

请查看我们单独的 [常见问题解答](FAQ.md)。

## 关于我们 (SixtyFPS)

我们对软件开发充满热情，涵盖 API 设计、跨平台软件开发以及用户界面组件等方面。我们的目标是让每个人在开发用户界面时都能乐在其中：从使用 Python、JavaScript、C++ 或 Rust 的开发者，到用户界面 / 用户体验（UI/UX）设计师，概莫能外。我们相信，软件是自然发展演进的，而保持其开源属性是维持这种发展的最佳方式。我们的团队成员分布在德国、芬兰和美国，我们采用远程办公模式。

### 最新动态

- 在 X/Twitter 上关注 [@slint_ui](https://twitter.com/slint_ui)。
- 在 Mastodon 上关注 [@slint@fosstodon.org](https://mastodon.social/@slint@fosstodon.org)。
- 在 LinkedIn 上关注 [@slint-ui](https://www.linkedin.com/company/slint-ui/)。
- 在 Bluesky 上关注 [@slint.dev](https://bsky.app/profile/slint.dev)。
- 订阅我们的 [YouTube 频道](https://www.youtube.com/@Slint-UI)。

### 联系我们

欢迎您加入 GitHub 上的 [讨论区](https://github.com/slint-ui/slint/discussions) 进行一般性的交流或提问。使用 GitHub 的 [问题](https://github.com/slint-ui/slint/issues) 板块来提交公开的建议或报告漏洞。

我们在 [Mattermost](https://chat.slint.dev) 上进行交流，欢迎您来关注或提出问题。

当然，您也可以通过发送电子邮件至 [info@slint.dev](mailto://info@slint.dev) 与我们私下联系。

[#1]: https://www.youtube.com/watch?v=_BDbNHrjK7g
[#2]: https://www.youtube.com/watch?v=NNNOJJsOAis
[#3]: https://www.youtube.com/watch?v=dkBwNocItGs
[#4]: https://slint.dev/resources/gallery_win_screenshot.png "Gallery"
[#5]: https://slint.dev/resources/gallery_mac_screenshot.png "Gallery"
[#6]: https://slint.dev/resources/gallery_linux_screenshot.png "Gallery"
[#7]: https://slint.dev/resources/printerdemo_screenshot.png "Printer Demo"
[#8]: https://slint.dev/demos/printerdemo/
[#9]: https://slint.dev/resources/puzzle_screenshot.png "Slide Puzzle"
[#10]: https://slint.dev/demos/slide_puzzle/
[#11]: https://slint.dev/resources/energy-monitor-screenshot.png "Energy Monitor Demo"
[#12]: https://slint.dev/demos/energy-monitor/
[#13]: https://slint.dev/resources/gallery_screenshot.png "Gallery Demo"
[#14]: https://slint.dev/demos/gallery/
[#15]: https://slint.dev/latest/docs/cpp
[#17]: https://github.com/slint-ui/slint-cpp-template
[#18]: https://img.shields.io/crates/v/slint
[#19]: https://crates.io/crates/slint
[#20]: https://slint.dev/latest/docs/rust/slint/
[#22]: https://youtu.be/WBcv4V-whHk
[#23]: https://github.com/slint-ui/slint-rust-template
[#24]: https://img.shields.io/npm/v/slint-ui
[#25]: https://www.npmjs.com/package/slint-ui
[#26]: https://slint.dev/latest/docs/node
[#28]: https://github.com/slint-ui/slint-nodejs-template
[#29]: ./demos/weather-demo/docs/img/desktop-preview.png "Weather Demo"
[#30]: https://slint.dev/demos/weather-demo/
[#31]: https://img.shields.io/pypi/v/slint
[#32]: https://pypi.org/project/slint/
[#33]: http://snapshots.slint.dev/master/docs/python/
[#34]: https://github.com/slint-ui/slint-python-template
