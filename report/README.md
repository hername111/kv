# 实验报告

- main.tex：报告正文，使用 XeLaTeX 编译。
- main.pdf：编译后生成的提交文件（不在仓库中预置）。
- figures/：建议放置真实 Web、终端和测试截图。

## 编译

在仓库根目录执行：

    cd report
    xelatex -interaction=nonstopmode -halt-on-error main.tex
    xelatex -interaction=nonstopmode -halt-on-error main.tex

第二次编译用于更新目录和交叉引用。提交前请：

1. 替换姓名、学号、分工、日期和仓库地址占位符。
2. 用真实截图替换正文中的四个占位框。
3. 检查 PDF 中是否有表格、代码或中文字体溢出。
4. 只提交最终 PDF，不以 Word 文件替代。

报告正文基于当前仓库实现和当前测试基线撰写。若代码或测试结果在提交前发生变化，应同步更新正文。

