#!/usr/bin/env python3
"""从 ECDICT（MIT）构建随包分发的离线核心词库 models/dict/ecdict.db。

ECDICT 全量 77 万词条（含大量专业词与词组），这里只保留常用的单词：
  - 有中文释义；
  - 柯林斯星级 / 牛津 3000 / 考试大纲标签（中考 … GRE）/ 语料库词频，任一即可；
  - 不含数字，长度 ≤ 24；词组（含空格）只保留核心词。
另外从每个原形的 exchange 字段（p 过去式 / d 过去分词 / i 现在分词 / 3 第三人称单数 /
s 复数 / r 比较级 / t 最高级）建“变形 → 原形”表，查 perceived 时能还原到 perceive。

用法：scripts/build-dict.py <ecdict.csv> <out.db>
表结构（es-translate/src/dict.rs 读取）：
  words(key PRIMARY KEY 小写, word, phonetic IPA, translation 多行, tags, collins, oxford, frq, exchange)
  forms(form PRIMARY KEY 小写, lemma 小写, kind 变形代码)
"""
import csv
import os
import re
import sqlite3
import sys

FORM_KINDS = "pdi3srt"


def num(x):
    try:
        return int(x)
    except ValueError:
        return 0


def rank(r):
    """词频排名（越小越常用）：当代语料库 frq 优先，其次 BNC；没有时为 0。"""
    return num(r["frq"]) or num(r["bnc"])


def is_core(r):
    return (
        num(r["collins"]) > 0
        or r["oxford"] == "1"
        or bool(r["tag"].strip())
        or 0 < rank(r) <= 60000
    )


def keep(r):
    w = r["word"]
    if not r["translation"].strip() or len(w) > 24 or re.search(r"\d", w):
        return False
    if " " in w or w.startswith(("-", "'")):
        return is_core(r)
    return is_core(r) or rank(r) > 0


def ipa(p):
    """ECDICT 音标的写法统一到标准 IPA：西里尔 ә → ə，' → ˈ，, → ˌ，: → ː。"""
    return (
        p.replace("ә", "ə").replace("'", "ˈ").replace(",", "ˌ").replace(":", "ː").strip()
    )


def score(r):
    """同一小写形式有多条（Apple / apple）时保留更常用、信息更全的一条。"""
    return (r["word"].islower(), is_core(r), num(r["collins"]), -rank(r) if rank(r) else -10**9)


def main(src, out):
    rows = [r for r in csv.DictReader(open(src, encoding="utf-8")) if keep(r)]
    words = {}
    for r in rows:
        key = r["word"].lower()
        if key not in words or score(r) > score(words[key]):
            words[key] = r

    forms = {}
    for key, r in sorted(words.items(), key=lambda kv: rank(kv[1]) or 10**9):
        for part in r["exchange"].split("/"):
            kind, _, form = part.partition(":")
            form = form.strip().lower()
            if kind in FORM_KINDS and form and form != key:
                forms.setdefault(form, (key, kind))

    if os.path.exists(out):
        os.remove(out)
    db = sqlite3.connect(out)
    db.executescript(
        """
        PRAGMA page_size = 4096;
        CREATE TABLE words (
            key TEXT PRIMARY KEY, word TEXT NOT NULL, phonetic TEXT NOT NULL,
            translation TEXT NOT NULL, tags TEXT NOT NULL, collins INTEGER NOT NULL,
            oxford INTEGER NOT NULL, frq INTEGER NOT NULL, exchange TEXT NOT NULL
        ) WITHOUT ROWID;
        CREATE TABLE forms (
            form TEXT PRIMARY KEY, lemma TEXT NOT NULL, kind TEXT NOT NULL
        ) WITHOUT ROWID;
        """
    )
    db.executemany(
        "INSERT INTO words VALUES (?,?,?,?,?,?,?,?,?)",
        [
            (
                key,
                r["word"],
                ipa(r["phonetic"]),
                r["translation"].replace("\\n", "\n").strip(),
                r["tag"].strip(),
                num(r["collins"]),
                1 if r["oxford"] == "1" else 0,
                rank(r),
                r["exchange"].strip(),
            )
            for key, r in words.items()
        ],
    )
    db.executemany("INSERT INTO forms VALUES (?,?,?)", [(f, l, k) for f, (l, k) in forms.items()])
    db.commit()
    db.execute("VACUUM")
    db.close()
    print(f"{out}: {len(words)} words, {len(forms)} forms, {os.path.getsize(out) / 1e6:.1f} MB")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
