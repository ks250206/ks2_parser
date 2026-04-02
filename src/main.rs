
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Config {
    input_path: PathBuf,
    output_dir: PathBuf,
    #[serde(default = "default_output_file_name")]
    output_file_name: String,
    #[serde(default)]
    auto_detect_offsets: bool,

    header_byte: usize,
    variable_header_byte: usize,
    data_header_byte: usize,
    data_skip_byte: usize,
    footer_byte: usize,

    values_per_record: usize,
    endianness: Endianness,
    #[serde(rename = "ADConverterScale")]
    ad_converter_scale: f64,
    #[serde(rename = "ADRangeCoefficient")]
    ad_range_coefficient: f64,
    #[serde(rename = "ADCoefficient")]
    ad_coefficient: f64,
    coefficient: ChannelCoefficient,
}

#[derive(Debug, Deserialize)]
struct ChannelCoefficient {
    #[serde(rename = "CH1")]
    ch1: f64,
    #[serde(rename = "CH2")]
    ch2: f64,
    #[serde(rename = "CH3")]
    ch3: f64,
    #[serde(rename = "CH4")]
    ch4: f64,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Endianness {
    Little,
    Big,
}

fn main() -> Result<()> {
    let mut config = load_config("config.toml")?;

    fs::create_dir_all(&config.output_dir)
        .with_context(|| format!("failed to create output dir: {}", config.output_dir.display()))?;

    let bytes = fs::read(&config.input_path)
        .with_context(|| format!("failed to read input file: {}", config.input_path.display()))?;

    if config.auto_detect_offsets {
        apply_auto_detected_offsets(&mut config, &bytes)?;
    }

    validate_config(&config)?;

    let data_region = extract_data_region(&bytes, &config)?;
    let records = parse_records(data_region, &config)?;

    write_combined_csv(&config, &records)?;

    println!("done");
    println!("input: {}", config.input_path.display());
    println!("records: {}", records.len());
    println!("variable_header_byte: {}", config.variable_header_byte);
    println!("data_header_byte: {}", config.data_header_byte);
    println!("footer_byte: {}", config.footer_byte);
    println!("output dir: {}", config.output_dir.display());
    println!("output file: {}", config.output_file_name);

    Ok(())
}

fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let text = fs::read_to_string(path.as_ref())
        .with_context(|| format!("failed to read config: {}", path.as_ref().display()))?;
    let config: Config = toml::from_str(&text).context("failed to parse TOML config")?;
    Ok(config)
}

fn default_output_file_name() -> String {
    "output.csv".to_string()
}

fn validate_config(config: &Config) -> Result<()> {
    if config.values_per_record != 4 {
        bail!(
            "this program expects values_per_record = 4, but got {}",
            config.values_per_record
        );
    }
    if config.ad_converter_scale == 0.0 {
        bail!("ADConverterScale must not be 0");
    }
    Ok(())
}

fn apply_auto_detected_offsets(config: &mut Config, bytes: &[u8]) -> Result<()> {
    config.variable_header_byte = parse_usize_after_crlf(bytes, 12)?;
    config.data_header_byte = parse_usize_after_crlf(bytes, 13)?;
    config.footer_byte = parse_usize_after_crlf(bytes, 14)?;
    Ok(())
}

fn parse_usize_after_crlf(bytes: &[u8], crlf_index_1based: usize) -> Result<usize> {
    let start = nth_crlf_end(bytes, crlf_index_1based)
        .with_context(|| format!("failed to find the {crlf_index_1based}th CRLF"))?;
    let field = bytes
        .get(start..start + 14)
        .with_context(|| format!("failed to read 14 bytes after the {crlf_index_1based}th CRLF"))?;
    let text = std::str::from_utf8(field)
        .with_context(|| format!("field after the {crlf_index_1based}th CRLF is not valid UTF-8"))?;
    let value = text
        .trim()
        .parse::<usize>()
        .with_context(|| format!("failed to parse integer from {:?}", text))?;
    Ok(value)
}

fn nth_crlf_end(bytes: &[u8], target_1based: usize) -> Option<usize> {
    let mut count = 0;
    let mut i = 0;

    while i + 1 < bytes.len() {
        if bytes[i] == b'\r' && bytes[i + 1] == b'\n' {
            count += 1;
            if count == target_1based {
                return Some(i + 2);
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    None
}

fn extract_data_region<'a>(bytes: &'a [u8], config: &Config) -> Result<&'a [u8]> {
    let start = config
        .header_byte
        .checked_add(config.variable_header_byte)
        .and_then(|v| v.checked_add(config.data_header_byte))
        .and_then(|v| v.checked_add(config.data_skip_byte))
        .context("overflow while calculating data start offset")?;

    let end = bytes
        .len()
        .checked_sub(config.footer_byte)
        .context("footer_byte is larger than input file size")?;

    if start > end {
        bail!(
            "invalid region: start offset ({start}) is greater than end offset ({end})"
        );
    }

    Ok(&bytes[start..end])
}

fn parse_records(data: &[u8], config: &Config) -> Result<Vec<[i32; 4]>> {
    let record_size = config.values_per_record * std::mem::size_of::<i32>();

    if record_size != 16 {
        bail!("record size must be 16 bytes, but got {record_size}");
    }

    if data.len() % record_size != 0 {
        bail!(
            "data length ({}) is not a multiple of record size ({})",
            data.len(),
            record_size
        );
    }

    let mut records = Vec::with_capacity(data.len() / record_size);

    for chunk in data.chunks_exact(record_size) {
        let mut values = [0_i32; 4];

        for i in 0..4 {
            let start = i * 4;
            let raw: [u8; 4] = chunk[start..start + 4]
                .try_into()
                .context("failed to convert 4-byte slice into array")?;

            values[i] = match config.endianness {
                Endianness::Little => i32::from_le_bytes(raw),
                Endianness::Big => i32::from_be_bytes(raw),
            };
        }

        records.push(values);
    }

    Ok(records)
}

fn write_combined_csv(config: &Config, records: &[[i32; 4]]) -> Result<()> {
    let path = config.output_dir.join(&config.output_file_name);
    let channel_coefficients = [
        config.coefficient.ch1,
        config.coefficient.ch2,
        config.coefficient.ch3,
        config.coefficient.ch4,
    ];
    let mut wtr = csv::Writer::from_path(&path)
        .with_context(|| format!("failed to open CSV for writing: {}", path.display()))?;
    wtr.write_record(["index", "ch1", "ch2", "ch3", "ch4"])
        .with_context(|| format!("failed to write header: {}", path.display()))?;

    for (index, record) in records.iter().enumerate() {
        let mut row = [String::new(), String::new(), String::new(), String::new(), String::new()];
        row[0] = index.to_string();

        for ch in 0..4 {
            let scaled_value = (record[ch] as f64 / config.ad_converter_scale)
                * config.ad_range_coefficient
                * config.ad_coefficient
                * channel_coefficients[ch];
            row[ch + 1] = scaled_value.to_string();
        }

        wtr.write_record(row)
            .with_context(|| format!("failed to write record {index} to {}", path.display()))?;
    }

    wtr.flush().context("failed to flush CSV writer")?;

    Ok(())
}
