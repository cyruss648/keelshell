# 首轮封存候选：非最终验收

原 stage 将 package-/prebuilt- 文件名误当目录过滤，漏了打包和清理回执。
封存命令自身 exit0，但随后工作区 verifier 以缺失 package-r1.json 失败，exit1。
原 manifest、archive 与当时 verifier 完整保留；不能作为最终验收。
父级修正仅过滤目录后使用新 stage 和根目录最终封存，未重放产品测试。
失败原件随后保存在交付补记，不覆盖本候选。
