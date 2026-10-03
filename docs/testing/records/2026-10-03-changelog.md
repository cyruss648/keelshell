# 变更日志生成验证

## 范围

验证本地脚本可以从固定 Git 历史生成版本段，并且发布流水线在标签构建时拒绝缺少对应版本段的仓库。

## 命令

```text
python scripts/changelog.py check --version 0.1.0
python scripts/changelog.py generate --version 0.1.0 --output /tmp/keelshell-changelog.md
```

脚本不访问网络，也不创建或移动 Git 标签。生成的临时文件仅用于比较，不替换仓库内的已审阅版本段。

## 边界

该记录证明脚本与 CI 校验逻辑可运行，不证明 GitHub Release 已发布，也不代表远端发布说明会自动更新本地未提交文件。发布仍由获得授权的标签流水线完成。
