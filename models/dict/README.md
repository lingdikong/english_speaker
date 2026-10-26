# ECDICT 离线词典（核心词库）

查词底座：随安装包分发，只读打开，查一次 < 1ms，**完全离线**。

| 文件 | 说明 |
|---|---|
| `ecdict.db` | 从 ECDICT 裁剪出的常用词（约 5.9 万词条 + 3.9 万条“变形 → 原形”），SQLite，约 7MB |

由 `scripts/build-dict.py` 从 [skywind3000/ECDICT](https://github.com/skywind3000/ECDICT) @ `ab22ad3` 的 `ecdict.csv` 生成，
保留：有中文释义，且有柯林斯星级 / 牛津 3000 / 考试大纲标签（中考 … GRE）/ 语料库词频之一的单词。
`scripts/fetch-models.sh` 会下载原始数据并重新生成。

字段：音标（统一为标准 IPA）、多行中文释义（`vt. 感觉, 认知`）、考试标签、柯林斯星级、牛津核心词、词频排名、词形变化。

## 许可

ECDICT：**MIT**，Copyright (c) Linwei（skywind3000）。全文见同目录 `LICENSE`。
