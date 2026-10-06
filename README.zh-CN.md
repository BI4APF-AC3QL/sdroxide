# SDRoxide 简体中文语言插件使用说明（Windows）

> SDRoxide 是一款面向业余无线电爱好者的 SDR 收发软件。本帖介绍的是第三方简体中文语言插件预览包的安装、切换和恢复方式。它不是 SDRoxide 官方发布，也不代表上游项目的支持或背书。
>
> **版本基线**：汉化插件 `0.2.2-preview`，适配上游 SDRoxide `1.6.9`。
>
> **GitHub 下载**：[v0.2.2-preview 发布页](https://github.com/BI4APF-AC3QL/sdroxide/releases/tag/v0.2.2-preview)。发布附件：[完整语言包 ZIP（314.1 MB）](https://github.com/BI4APF-AC3QL/sdroxide/releases/download/v0.2.2-preview/SDRoxide-zh-CN-package-20261006-145929.zip)；[一键安装／恢复 EXE（317.6 MB）](https://github.com/BI4APF-AC3QL/sdroxide/releases/download/v0.2.2-preview/SDRoxide-zh-CN-Installer-Restore-20261006.exe)。[汉化源码与 README](https://github.com/BI4APF-AC3QL/sdroxide/tree/zh-cn-language-plugin)。下载 ZIP 后完整解压，并保留同级 `plugins` 目录；也可以先阅读下文的安装步骤。

## 一、语言插件是什么

汉化文本、字体和手册以语言插件文件提供；为加载插件，需要配套的汉化宿主。切换器会先备份你选择的原版 EXE，再将它替换为随包提供的兼容宿主，并安装 `plugins\zh-CN`。上游原版 `sdroxide.exe` 本身不能直接加载此插件。发布包还包含中文字体、中文手册、恢复工具和对应源码。

界面中的动态呼号、频率、模式缩写、设备型号及协议字段会按原样保留，避免影响电台操作和数据识别。当前包随附的中文手册覆盖全文；关闭插件后，界面与手册回到英文。

## 二、安装与启用

1. 完整解压发布包到一个普通文件夹，并保留同级的 `plugins` 目录。不要只单独拷贝 EXE。
2. 关闭正在运行的 SDRoxide。
3. 双击 `SdroxideSwitcher.exe`。
4. 选择原版 `sdroxide.exe` 的完整路径，然后点击“检查路径”。
5. 点击“启用简体中文”。工具会备份原 EXE 并放置随包提供的语言插件。
6. 启动 SDRoxide。若界面仍是英文，进入 **SETTINGS（设置）→ UI（界面）→ Language packs（语言包）**，选择 **简体中文 / Simplified Chinese**。

也可以不替换已有安装：在完整解压后的文件夹中直接运行 `sdroxide-zh-CN.exe`，同时保留旁边的 `plugins` 文件夹。

## 三、恢复原版

先关闭 SDRoxide，再启动 `SdroxideSwitcher.exe`，选择同一个 `sdroxide.exe` 路径并点击“恢复原版”。切换器会恢复安装前的原始 EXE，并移除由本工具安装的语言包。建议在操作前不要手动删除切换器创建的备份文件。

## 四、常见问题

**打开后还是英文怎么办？**

确认启动的是发布包内的汉化宿主，而不是未修改的上游原版；确认整个 `plugins\zh-CN` 文件夹与 EXE 一起保留；再到 **SETTINGS（设置）→ UI（界面）→ Language packs（语言包）** 检查已选中“简体中文 / Simplified Chinese”。

**能否把语言插件复制到任意 SDRoxide 版本？**

不能。此预览包的插件需要配套汉化宿主，和上游原版或其他版本不保证兼容。上游升级后应等待对应适配版，不建议覆盖安装目录中的其他文件。

**这是否是官方中文版？**

不是。这是社区制作的非官方简体中文插件预览版。SDRoxide 官方网站与上游项目仍以官方发布为准：<https://sdroxide.com/>、<https://github.com/dividebysandwich/sdroxide>。

## 五、界面展示

以下图片来自 SDRoxide **官方网站**，用于展示上游软件功能；它们不是本汉化预览版的实机截图，实际翻译效果以随包程序为准。

### 主界面：频谱与瀑布图

![SDRoxide 官方网站主界面截图，包含顶部控制栏、频谱和瀑布图](https://sdroxide.com/assets/img/01-main-window.jpg)

### FT8 解码窗口

![SDRoxide 官方网站 FT8 窗口截图，显示解码台站列表和地图](https://sdroxide.com/assets/img/07-ft8-panel.png)

### 浏览器远程界面

![SDRoxide 官方网站 Web 客户端截图](https://sdroxide.com/assets/img/13-web-client.png)

图片来源：<https://sdroxide.com/>（SDRoxide 官方网站）。请以官方站点现行版权与使用条款为准。

## 六、使用边界与反馈

这是预览版，原生设置和帮助页仍需完成完整截图验收；安装切换器与程序启动／恢复已有自动化检查。请先在无电台或接收测试环境检查界面，不要把这份说明理解为发射许可。本次版本没有进行发射或 PTT 测试。发现错译、显示异常或安装问题时，请在本帖附上 SDRoxide 版本、Windows 版本和问题界面截图；不要附带密码、密钥或其他个人配置文件。

---

**发布版本**：`v0.2.2-preview`（适配 SDRoxide `1.6.9`）。下载文件：`SDRoxide-zh-CN-package-20261006-145929.zip` 与 `SDRoxide-zh-CN-Installer-Restore-20261006.exe`。