//! 内核版本解析与 Landlock ABI floor 推导（纯函数）。
//!
//! `uname -r` 的发行版串（如 `6.18.54.1-microsoft-standard-WSL2`）不是严格
//! semver（四段 + 后缀），手写解析而非引入 semver crate。
//! ABI floor 映射依据：内核文档 userspace-api/landlock 的 ABI 版本表
//! （v1=5.13 … v4=6.7 引入网络 access rights）+ 本机实测校准
//! （spike① #16 附录 A：6.18.54.1 探针返回 ABI 7）。

/// 解析后的内核版本（主.次.修订；后续段与后缀忽略，向下比较足够）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct KernelVersion {
    /// 主版本。
    pub major: u32,
    /// 次版本。
    pub minor: u32,
    /// 修订版本。
    pub patch: u32,
}

impl KernelVersion {
    /// 解析 `uname -r` 输出（取首个 `数字.数字.数字` 前缀）。
    ///
    /// # Errors
    ///
    /// 找不到合法的 `x.y.z` 三段版本前缀时返回 `Err(String)`
    /// （错误描述供探测报告的 detail 使用）。
    pub fn parse(release: &str) -> Result<Self, String> {
        let mut numbers: Vec<u32> = Vec::with_capacity(3);
        let mut current = String::new();
        let mut collected = 0usize;
        for ch in release.chars() {
            if ch.is_ascii_digit() {
                current.push(ch);
            } else if ch == '.' && !current.is_empty() {
                match current.parse() {
                    Ok(n) => {
                        numbers.push(n);
                        collected += 1;
                        current.clear();
                        if collected == 3 {
                            break;
                        }
                    }
                    Err(_) => return Err(format!("version segment too large in {release:?}")),
                }
            } else {
                break;
            }
        }
        if !current.is_empty() && collected < 3 {
            match current.parse() {
                Ok(n) => {
                    numbers.push(n);
                    collected += 1;
                }
                Err(_) => return Err(format!("version segment too large in {release:?}")),
            }
        }
        if collected < 3 {
            return Err(format!("no valid x.y.z prefix in {release:?}"));
        }
        Ok(Self {
            major: numbers[0],
            minor: numbers[1],
            patch: numbers[2],
        })
    }

    /// 是否满足 Landlock 网络下限（`qn_config::WSL_MIN_KERNEL_SEMVER`，即 6.7.0）。
    #[must_use]
    pub fn meets_landlock_net_floor(&self) -> bool {
        let floor = Self::parse(qn_config::WSL_MIN_KERNEL_SEMVER).unwrap_or(Self {
            major: 6,
            minor: 7,
            patch: 0,
        });
        self >= &floor
    }

    /// Landlock ABI floor：该内核修订线保证的最低 ABI 版本（0 = 不支持）。
    #[must_use]
    pub fn landlock_abi_floor(&self) -> u32 {
        match (self.major, self.minor) {
            (m, _) if m < 5 => 0,
            (5, minor) if minor >= 19 => 2,
            (5, 13..=18) => 1,
            (6, minor) if minor >= 18 => 7,
            (6, 12..=17) => 6,
            (6, 10..=11) => 5,
            (6, 7..=9) => 4,
            (6, 2..=6) => 3,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn it_parses_wsl_release_strings() {
        // spike① 实测样本。
        assert_eq!(
            KernelVersion::parse("6.18.54.1-microsoft-standard-WSL2").unwrap(),
            KernelVersion {
                major: 6,
                minor: 18,
                patch: 54
            }
        );
        assert_eq!(
            KernelVersion::parse("6.6.87.2-microsoft-standard-WSL2").unwrap(),
            KernelVersion {
                major: 6,
                minor: 6,
                patch: 87
            }
        );
    }

    #[test]
    fn it_parses_plain_semver() {
        assert_eq!(
            KernelVersion::parse(qn_config::WSL_MIN_KERNEL_SEMVER).unwrap(),
            KernelVersion {
                major: 6,
                minor: 7,
                patch: 0
            }
        );
    }

    #[test]
    fn it_rejects_garbage() {
        assert!(KernelVersion::parse("").is_err());
        assert!(KernelVersion::parse("linux").is_err());
        assert!(
            KernelVersion::parse("6.18").is_err(),
            "two segments are not enough"
        );
    }

    #[test]
    fn it_checks_landlock_net_floor() {
        assert!(
            KernelVersion {
                major: 6,
                minor: 7,
                patch: 0
            }
            .meets_landlock_net_floor()
        );
        assert!(
            KernelVersion {
                major: 6,
                minor: 18,
                patch: 54
            }
            .meets_landlock_net_floor()
        );
        assert!(
            !KernelVersion {
                major: 6,
                minor: 6,
                patch: 87
            }
            .meets_landlock_net_floor()
        );
    }

    #[test]
    fn it_maps_abi_floor_from_kernel_docs() {
        // 内核文档 ABI 版本表 + spike① 实测校准（6.18.54.1 → 7）。
        assert_eq!(
            KernelVersion {
                major: 5,
                minor: 13,
                patch: 0
            }
            .landlock_abi_floor(),
            1
        );
        assert_eq!(
            KernelVersion {
                major: 6,
                minor: 2,
                patch: 0
            }
            .landlock_abi_floor(),
            3
        );
        assert_eq!(
            KernelVersion {
                major: 6,
                minor: 6,
                patch: 87
            }
            .landlock_abi_floor(),
            3
        );
        assert_eq!(
            KernelVersion {
                major: 6,
                minor: 7,
                patch: 0
            }
            .landlock_abi_floor(),
            4
        );
        assert_eq!(
            KernelVersion {
                major: 6,
                minor: 18,
                patch: 54
            }
            .landlock_abi_floor(),
            7
        );
        assert_eq!(
            KernelVersion {
                major: 4,
                minor: 19,
                patch: 0
            }
            .landlock_abi_floor(),
            0
        );
    }
}
