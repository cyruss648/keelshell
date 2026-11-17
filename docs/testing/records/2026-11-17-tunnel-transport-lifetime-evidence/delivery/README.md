# 本地转发生命周期：远端交付与回收

工程 `cdf1e171f2f9b179f09db230f4739696d02fe80d` 已普通推送到
`origin/feature/editor-unicode-codec-20261114`。
`feature/result.json`保存本地、tracking、远端三方SHA相等及真实命令回执；
交付控制 `0/reaped/6.437592s`，未强推或改写main。
回执生成时尚有本目录的自有docs文件待提交，未把该时刻声称为干净工作区。

`feature/git-binding.log`逐件核对769工程输入及两份归档478成员与工程提交的
完整Git blob相等。可在有此Git对象的checkout中只读重算：

```sh
python3 -B docs/testing/records/2026-11-17-tunnel-transport-lifetime-evidence/verify_git_binding.py \
  --commit cdf1e171f2f9b179f09db230f4739696d02fe80d
```

此命令不访问target、网络或运行产品测试。它不推断新的原生或发行结果。

`cleanup-result.json`保存实际回收前后保护信息。
6个自有目录／478文件／11,059,237逻辑字节先与Git内原件及完整文件集相等，
才被删除；其中包含前批剩余finalize目录。唯一target的目录身份与正常旧包
完整5文件摘要保持。另2份reviewer测试二进制副本已核对原target后回收，
其63,816,528逻辑字节与原件清理分开记录，见主归档的`prebuilt-copy-cleanup.json`。
不将长度合计写成物理磁盘空间差值，不删除其他任务资源。

5位本批作者／reviewer均已关闭，没有新增用户chat、worktree或容器。
原work路径已回收，维护入口是主归档、精确CI归档和当前交接。

`source-ci-initial/`是对cdf1e171的真实GitHub API读取：
Quality44020265369、Windows API44020265273、Windows SSH44020265248
均为attempt1，初态in_progress。它不包含新的CI成功、artifact或native结论；
继续跟踪同一run，不以旧工程成功或文档HEAD代签。

`source-ci-followup/`后次读取显示两个Windows工作流completed/success，
Quality的三个job仍in_progress、未观察失败job。它仍是API状态／步骤元数据，
未取得新artifact或签署其内容；初态原件保持原字节。
