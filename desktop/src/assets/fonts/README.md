# 霞鹜文楷 GB

来源：`https://github.com/lxgw/LxgwWenKaiGB`，版本 `v1.530`。

应用使用 Regular，保留完整字符集，使用 fontTools 4.67.0 和 Brotli 1.2.0 将上游 TTF 无损转换为 WOFF2，以减少内嵌字体体积。字体名称保持不变，许可见同目录 `OFL.txt`。不需要联网加载字体。

上游 `LXGWWenKaiGB-Regular.ttf` SHA-256：

`5561d48097d92e5ab99fe7b1459d381bbcdff533bfc479207b089c6860c3990e`

重建方式：从固定版本下载原 TTF，安装 `fonttools[woff]` 后执行：

```python
from fontTools.ttLib import TTFont

font = TTFont("LXGWWenKaiGB-Regular.ttf")
font.flavor = "woff2"
font.save("LXGWWenKaiGB-Regular.woff2")
```
