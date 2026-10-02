**语言 / Languages：** [简体中文](README.md) · [English](docs/README.en.md)

# 烟雨 OTA 工具箱（YanRain OTA ToolBox）

<img src="YanRainOTAToolBox/src-tauri/icons/icon.png" width="112" alt="YanRain OTA ToolBox Logo">

一款面向 Android 设备的一站式可视化玩机工具箱：刷机、Root、投屏、固件管理，全部图形化操作，支持 Windows。

[![Release](https://img.shields.io/badge/Release-v1.1.2-e56565)](https://github.com/RegularsYr7/YanRainOTAToolBox/releases)
[![Website](https://img.shields.io/badge/Website-rainyweb.cn-e56565?logo=googlechrome&logoColor=white)](https://www.rainyweb.cn/)
[![Telegram](https://img.shields.io/badge/Telegram-%40rainyotatoolbox-26A5E4?logo=telegram&logoColor=white)](https://t.me/rainyotatoolbox)
[![License](https://img.shields.io/badge/License-GPL--3.0-e56565)](LICENSE)

## 支持与赞助

如果这个工具箱帮到了你，欢迎请我喝杯奶茶。项目从开发、测试到持续维护都需要投入不少时间，你的支持是我继续更新的最大动力。

一块两块不嫌少，十块八块不嫌多；无论多少，心意都弥足珍贵。感谢每一位支持者！

| 爱发电 | 支付宝 | 微信 |
| --- | --- | --- |
| <img src="assets/sponsor/afdian.jpg" width="240" alt="爱发电"> | <img src="assets/sponsor/alipay.jpg" width="240" alt="支付宝"> | <img src="assets/sponsor/wechat.png" width="240" alt="微信"> |

## 功能一览

### 1. 设备管理

- 自动发现 USB / 无线连接的 Android 设备
- 设备详情、电池 / 存储 / 内存实时监控
- 一键有线转无线调试

### 2. 刷机工具

- **基础刷写** — Fastboot 单分区刷入、OEM 解锁 / 上锁
- **批量刷写** — TXT 脚本解析、ADB Sideload、Fastboot Update
- **分区管理** — 格式化、提取、备份、整机全量备份
- **深度刷写** — Qualcomm EDL 9008 模式全盘刷入
- **小米助手** — ROM 查询下载、HyperOS 数据源、在线一键刷机
- **一加线刷** — ROM 解密、Super 合并、EDL 全盘刷入

### 3. Root 修补

- 支持 Magisk / KernelSU / KernelSU-Next / APatch 四大方案
- 桌面端离线修补 boot 镜像，无需手机安装管理器

### 4. 应用管理

安装、卸载、冻结与导出应用，支持搜索筛选和批量操作。

### 5. 模块管理

可视化管理 Magisk 模块，支持安装、启用、禁用与删除。

### 6. 屏幕投射

集成 scrcpy，实现设备投屏、按键模拟与截屏。

### 7. 文件传输

浏览设备文件，支持上传、下载并显示实时进度。

### 8. 固件解包

解析并提取 Payload.bin，支持本地文件与 HTTP Range 在线解析。

### 9. 下载中心

内置 aria2 多协议下载引擎，支持 HTTP、BT、磁力链接与断点续传。

### 10. 固件中心

聚合多品牌 ROM 资源，支持蓝奏云链接解析与一键下载。

### 11. 设备调控

调整分辨率、DPI、电池、动画与状态栏等系统参数。

### 12. 视觉与语言

- 多主题：亮色 / 暖色 / 暗色 / 跟随系统
- 中英双语界面

## 下载与安装

前往 [Releases](https://github.com/RegularsYr7/YanRainOTAToolBox/releases) 下载最新版本：

| 架构 | 安装包 | 免安装版 |
| --- | --- | --- |
| x64 | `_x64-setup.exe` | `_x64_portable.zip` |

> 目前仅支持 Windows。建议安装到纯英文路径，中文路径可能导致刷机等功能异常。

## 构建

环境要求：Windows 10/11、Node.js 20+、pnpm、Rust stable。在 `YanRainOTAToolBox/` 目录下执行：

```powershell
pnpm install
pnpm tauri:dev
pnpm tauri:build
```

## 常见问题

**Q: 连接设备后显示 "Unauthorized"？**
请在手机上点击“允许 USB 调试”。若没有弹窗：撤销授权 → 重新插拔 → 再次授权。

**Q: 无线调试连不上？**
确保手机和电脑在同一局域网，先通过 USB 连接，工具会自动切换为无线模式。

**Q: Fastboot 刷写提示 "device locked"？**
Bootloader 未解锁，请先使用 OEM 解锁功能。

**Q: KernelSU 修补提示 "KMI not matched"？**
设备内核版本不在预置列表中，可手动指定 `.ko` 文件。

**Q: scrcpy 投屏黑屏？**
MIUI / HyperOS 需开启“USB 调试（安全设置）”，或尝试降低分辨率 / 帧率。

## 相关链接

- 反馈建议：[Issues](https://github.com/RegularsYr7/YanRainOTAToolBox/issues)
- 酷安：[@烟雨ovo](http://www.coolapk.com/u/17411754)
- 文档：[docs](./docs)

## 许可证

本项目采用 [GNU General Public License v3.0](./LICENSE)（GPL-3.0）授权。
