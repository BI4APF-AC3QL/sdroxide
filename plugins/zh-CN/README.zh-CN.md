# SDRoxide 简体中文语言插件

当前同步基于 SDRoxide 1.6.9 上游 main，提交 9addfec60f0679a63f14c027d277978f0260ffbb。语言包版本为 0.2.3-preview，插件仍以独立资源形式工作，不改写上游源文案。汉化源码只在专用工作区维护；原始 checkout 保持未修改。

语言目录包含 5,348 条 UI 文本，使用 Noto Sans SC 字体（OFL 许可，见 fonts/OFL.txt）。本次更新补入卫星 SSTV 链路名称和模式译文，适配 UTC 与本地过境时间格式及其动态占位符，并补译 ISS SSTV、CW MIDI 电键和按电台保存 WSPR 参数的手册内容。呼号、协议名、频率、单位、地址、消息正文及用户输入保持原值；插件关闭或文本键缺失时回退英文。

F1 中文手册现有 248 个翻译片段，源文件哈希已更新到同步后的 docs/USER_MANUAL.md。手册资源缺失、源文档变化或结构不兼容时，帮助内容会回退英文。

Windows 软件测试结果：sdroxide-ui 801 项通过、着色器检查 4 项通过；WSPR 每电台配置 1 项通过；ICOM 网络模拟 49 项单元测试、18 项回环测试及 1 项文档测试通过；类型层 497 项通过。射频引擎测试未能构建，因为环境未安装 SoapySDR/PothosSDR；格式检查因未安装 rustfmt 组件而未执行。未连接实际电台，未进行实机发射或 PTT 操作。完整原生窗口视觉验收和当前上游 Windows EXE／安装恢复包重建仍待完成。

manifest.json、translations.csv、integration-report.json 和 validation.json 记录了语言资源与本次同步证据。
