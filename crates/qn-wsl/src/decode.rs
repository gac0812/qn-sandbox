//! wsl.exe 输出解码：UTF-16LE / UTF-8 双格式识别（纯安全 Rust）。
//!
//! 事实依据（spike① 实测，2026-10-24）：`wsl.exe` 自身的 UI 输出
//! （`--version`、`-l -v`）经管道时是 UTF-16LE（字节流含 NUL 交错）；
//! 而 `wsl -d <distro> -- <cmd>` 是 Linux 侧输出的 UTF-8 透传。
//! `qn-infra::process` 按字节捕获，解码职责归本模块。

/// 识别并解码 wsl.exe 输出：含 NUL 且长度为偶 → UTF-16LE；否则按 UTF-8 lossy。
#[must_use]
pub fn decode_wsl_output(bytes: &[u8]) -> String {
    if looks_like_utf16(bytes) {
        decode_utf16_lossy(bytes)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// 启发式：UTF-16LE 的 ASCII 段呈现为 `X 0x00 X 0x00` 交错——
/// 偶数长度、NUL 全部落在奇数位、且 NUL 占比高。
fn looks_like_utf16(bytes: &[u8]) -> bool {
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return false;
    }
    let mut nul_odd = 0usize;
    let mut nul_total = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == 0 {
            nul_total += 1;
            if i % 2 == 1 {
                nul_odd += 1;
            }
        }
    }
    if nul_total == 0 {
        return false;
    }
    // ASCII 主导的 UTF-16LE：NUL 全在奇数位且占约一半；带 BOM 直接判定。
    bytes.starts_with(&[0xFF, 0xFE]) || (nul_odd == nul_total && nul_total * 4 >= bytes.len())
}

/// UTF-16LE 解码（lossy：非法代理对替换为 U+FFFD）。
fn decode_utf16_lossy(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let (body, _had_bom) = match units.first() {
        Some(0xFEFF) => (&units[1..], true),
        _ => (&units[..], false),
    };
    char::decode_utf16(body.iter().copied())
        .map(|r| r.unwrap_or('\u{FFFD}'))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn it_decodes_utf16_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(utf16("WSL 版本: 3.0.3.0"));
        assert_eq!(decode_wsl_output(&bytes), "WSL 版本: 3.0.3.0");
    }

    #[test]
    fn it_decodes_utf16_without_bom_ascii_interleaved() {
        // wsl -l -v 的典型形态：ASCII 交错 NUL（spike① 实测）。
        assert_eq!(
            decode_wsl_output(&utf16("NAME STATE VERSION")),
            "NAME STATE VERSION"
        );
    }

    #[test]
    fn it_passes_utf8_through_unchanged() {
        // distro-exec 透传路径：UTF-8 Linux 输出（含中文）不做任何变换。
        let src = "6.18.54.1-microsoft-standard-WSL2\n内核正常\n";
        assert_eq!(decode_wsl_output(src.as_bytes()), src);
    }

    #[test]
    fn it_does_not_misclassify_plain_utf8_with_odd_nul() {
        // UTF-8 里偶发的单个 NUL（偶数位）不应触发 UTF-16 路径。
        let mut bytes = b"plain utf8".to_vec();
        bytes.push(0);
        // 长度 12（偶）但 NUL 在偶数位 → nul_odd != nul_total → 按 UTF-8。
        assert_eq!(decode_wsl_output(&bytes), "plain utf8\0");
    }

    #[test]
    fn it_decodes_lone_surrogate_to_replacement() {
        // 单测私有解码器：合法 UTF-16 单元序列中的孤立代理对 → U+FFFD，不 panic。
        let units: Vec<u16> = vec![0x0041, 0xD800, 0x0042]; // 'A', 孤立 high surrogate, 'B'
        let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(decode_utf16_lossy(&bytes), "A\u{FFFD}B");
    }

    #[test]
    fn it_does_not_claim_lone_surrogate_streams_as_utf16() {
        // 启发式边界（按设计）：孤立代理对的低字节 0x00 落在偶位，全奇位 NUL
        // 判据不满足 → 按 UTF-8 处理。启发式只对 wsl.exe 的 ASCII-UI 输出
        // 形态负责，不为任意字节流兜底。
        let mut bytes = utf16("A");
        bytes.extend_from_slice(&0xD800u16.to_le_bytes());
        bytes.extend(utf16("B"));
        assert_eq!(
            decode_wsl_output(&bytes),
            String::from_utf8_lossy(&bytes).into_owned()
        );
    }
}
