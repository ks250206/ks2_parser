# ks2_parser

`ks2` バイナリを読み込み、4ch のデータを 1 つの CSV に変換する Rust ツールです。

## 機能

- `ks2` ファイルから 4ch 分の `i32` データを読み込み
- `index,ch1,ch2,ch3,ch4` 形式の単一 CSV を出力
- 出力値を以下の式で変換

```text
raw / ADConverterScale * ADRangeCoefficient * ADCoefficient * 各chのcoefficient
```

- `variable_header_byte`、`data_header_byte`、`footer_byte` の自動判定モードに対応
- 出力ファイル名を `config.toml` から変更可能

## 実行方法

`config.toml` を編集してから実行します。

```bash
cargo run
```

## config.toml

例:

```toml
input_path = "Test0006.ks2"
output_dir = "out"
output_file_name = "result.csv"
auto_detect_offsets = true

header_byte = 256
variable_header_byte = 2890
data_header_byte = 13452
data_skip_byte = 12
footer_byte = 0

values_per_record = 4
endianness = "little"

ADConverterScale = 2099200002
ADRangeCoefficient = 5000
ADCoefficient = 256

[coefficient]
CH1 = 1.05
CH2 = 1.13
CH3 = 1.04
CH4 = 1.12
```

### 主な設定項目

- `input_path`: 入力する `.ks2` ファイル
- `output_dir`: CSV の出力先ディレクトリ
- `output_file_name`: 出力 CSV ファイル名
- `auto_detect_offsets`: `true` のときオフセットを自動判定
- `header_byte`: データ開始位置の基準ヘッダ長
- `variable_header_byte`: 可変ヘッダ長
- `data_header_byte`: データヘッダ長
- `data_skip_byte`: データ本体の前に追加でスキップするバイト数
- `footer_byte`: ファイル末尾から除外するバイト数
- `values_per_record`: 1 レコードあたりの値数。現在は `4` 固定
- `endianness`: `little` または `big`
- `ADConverterScale`, `ADRangeCoefficient`, `ADCoefficient`: 出力値の変換係数
- `[coefficient]`: ch ごとの補正係数

## 自動判定モード

`auto_detect_offsets = true` のとき、入力ファイル内の `CRLF (0D 0A)` を数えて以下の位置から 14 バイトを読み取り、ASCII 数字として解釈します。

- 12 個目の `CRLF` の直後 14 バイト: `variable_header_byte`
- 13 個目の `CRLF` の直後 14 バイト: `data_header_byte`
- 14 個目の `CRLF` の直後 14 バイト: `footer_byte`

`auto_detect_offsets = false` のときは、`config.toml` に書かれた値をそのまま使います。

## 出力形式

出力 CSV のヘッダは以下です。

```csv
index,ch1,ch2,ch3,ch4
```

各行は 1 レコード分のデータです。

## 注意

- 現在は 4ch 固定です
- `values_per_record` が `4` 以外だとエラーになります
- `ADConverterScale = 0` は無効です
