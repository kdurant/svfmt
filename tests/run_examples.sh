#!/usr/bin/env bash
# 示例 golden 测试：对 examples/ 下每个有 *_expected.sv 的输入文件运行 svfmt，
# 并与期望输出对比，汇总 PASS/FAIL。任一失败则退出码非 0。
#
# 等价于 design.md "测试" 一节的逐条命令，统一脚本化处理。
#
# 用法：
#   ./tests/run_examples.sh                 # 运行全部示例
#   ./tests/run_examples.sh test1 bsg       # 只运行指定示例（不含扩展名）
#
# 环境变量：
#   SVFMT  指定 svfmt 可执行文件路径（默认 ./target/release/svfmt）

set -u
cd "$(dirname "$0")/.." || exit 1

SVFMT=${SVFMT:-./target/release/svfmt}

if [ ! -x "$SVFMT" ]; then
    echo "未找到 $SVFMT，先构建 release 版本..." >&2
    cargo build --release || exit 1
fi

# 可选：只测试部分示例（按文件名，不含扩展名）
select_names=${*:-}

pass=0
fail=0
for f in examples/*.sv; do
    case "$f" in
        *_expected.sv | *_tmp.sv) continue ;;
    esac
    name=${f%.sv}
    name=${name#examples/}
    if [ -n "$select_names" ]; then
        found=0
        for sel in $select_names; do
            [ "$sel" = "$name" ] && found=1
        done
        [ "$found" -eq 1 ] || continue
    fi
    exp="examples/${name}_expected.sv"
    [ -f "$exp" ] || continue
    tmp=$(mktemp)
    if "$SVFMT" "examples/$name.sv" -o "$tmp" 2>/dev/null; then
        if diff -q "$tmp" "$exp" >/dev/null 2>&1; then
            echo "PASS  $name"
            pass=$((pass + 1))
        else
            echo "FAIL  $name"
            diff -u "$exp" "$tmp" | head -40
            fail=$((fail + 1))
        fi
    else
        echo "ERROR $name"
        fail=$((fail + 1))
    fi
    rm -f "$tmp"
done

echo "----"
echo "通过 $pass，失败 $fail"
[ "$fail" -eq 0 ]
