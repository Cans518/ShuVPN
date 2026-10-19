# 应用图标

`icon.svg` 是矢量源文件，`icon.png` 和 `icon.ico` 为分发资源。蓝底盾牌与界面品牌一致；托盘使用 `src/tray.rs` 的小尺寸实心状态图标，独立显示连接状态。

从仓库根目录重新生成 PNG：

```bash
npm run tauri --prefix desktop -- icon src-tauri/icons/icon.svg --output ../build/icons --png 512
```

安装 Pillow 后，在仓库根目录执行以下 Python 代码生成分发文件：

```python
from PIL import Image

image = Image.open("build/icons/512x512.png")
image.save("desktop/src-tauri/icons/icon.png")
image.save(
    "desktop/src-tauri/icons/icon.ico",
    sizes=[(size, size) for size in (16, 24, 32, 48, 64, 128, 256)],
)
```
