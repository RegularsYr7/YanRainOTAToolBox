use std::collections::HashMap;
use tracing::warn;

use crate::domain::command::RiskLevel;
use crate::infra::errors::AppError;

/// 命令安全策略
///
/// 使用逻辑命令白名单映射，拒绝任意原始 shell 拼接
pub struct SafetyPolicy {
    /// 命令白名单：逻辑命令名 -> (实际命令参数, 风险等级)
    commands: HashMap<String, CommandDef>,
}

struct CommandDef {
    /// ADB 实际参数前缀
    adb_args: Vec<String>,
    /// 是否允许追加用户参数
    allow_extra_args: bool,
    /// 风险等级
    risk_level: RiskLevel,
    /// 描述
    #[allow(dead_code)]
    description: String,
}

impl SafetyPolicy {
    pub fn new() -> Self {
        let mut commands = HashMap::new();

        // === 安全命令 ===
        register(
            &mut commands,
            "LIST_DEVICES",
            &["devices", "-l"],
            false,
            RiskLevel::Safe,
            "列出所有连接的设备",
        );
        register(
            &mut commands,
            "ADB_VERSION",
            &["version"],
            false,
            RiskLevel::Safe,
            "查看 ADB 版本",
        );
        register(
            &mut commands,
            "GET_STATE",
            &["get-state"],
            false,
            RiskLevel::Safe,
            "获取设备状态",
        );
        register(
            &mut commands,
            "GET_SERIALNO",
            &["get-serialno"],
            false,
            RiskLevel::Safe,
            "获取设备序列号",
        );
        register(
            &mut commands,
            "GET_DEVPATH",
            &["get-devpath"],
            false,
            RiskLevel::Safe,
            "获取设备路径",
        );

        // === 普通命令 ===
        register(
            &mut commands,
            "SHELL",
            &["shell"],
            true,
            RiskLevel::Normal,
            "执行 shell 命令",
        );
        register(
            &mut commands,
            "INSTALL",
            &["install"],
            true,
            RiskLevel::Normal,
            "安装 APK",
        );
        register(
            &mut commands,
            "UNINSTALL",
            &["uninstall"],
            true,
            RiskLevel::Normal,
            "卸载应用",
        );
        register(
            &mut commands,
            "PUSH",
            &["push"],
            true,
            RiskLevel::Normal,
            "推送文件到设备",
        );
        register(
            &mut commands,
            "PULL",
            &["pull"],
            true,
            RiskLevel::Normal,
            "从设备拉取文件",
        );
        register(
            &mut commands,
            "LOGCAT",
            &["logcat"],
            true,
            RiskLevel::Normal,
            "查看日志",
        );
        register(
            &mut commands,
            "BUGREPORT",
            &["bugreport"],
            true,
            RiskLevel::Normal,
            "获取 bug report",
        );
        register(
            &mut commands,
            "CONNECT",
            &["connect"],
            true,
            RiskLevel::Normal,
            "连接网络设备",
        );
        register(
            &mut commands,
            "DISCONNECT",
            &["disconnect"],
            true,
            RiskLevel::Normal,
            "断开网络设备",
        );
        register(
            &mut commands,
            "TCPIP",
            &["tcpip"],
            true,
            RiskLevel::Normal,
            "切换到 TCP/IP 模式",
        );

        // === 危险命令（需二次确认）===
        register(
            &mut commands,
            "REBOOT",
            &["reboot"],
            false,
            RiskLevel::Dangerous,
            "重启设备",
        );
        register(
            &mut commands,
            "REBOOT_RECOVERY",
            &["reboot", "recovery"],
            false,
            RiskLevel::Dangerous,
            "重启到 recovery",
        );
        register(
            &mut commands,
            "REBOOT_BOOTLOADER",
            &["reboot", "bootloader"],
            false,
            RiskLevel::Dangerous,
            "重启到 bootloader",
        );
        register(
            &mut commands,
            "REMOUNT",
            &["remount"],
            false,
            RiskLevel::Dangerous,
            "重新挂载系统分区",
        );
        register(
            &mut commands,
            "SIDELOAD",
            &["sideload"],
            true,
            RiskLevel::Dangerous,
            "刷入 OTA 包",
        );
        register(
            &mut commands,
            "REBOOT_FASTBOOTD",
            &["reboot", "fastboot"],
            false,
            RiskLevel::Dangerous,
            "重启到 FastbootD",
        );
        register(
            &mut commands,
            "REBOOT_DOWNLOAD",
            &["reboot", "download"],
            false,
            RiskLevel::Dangerous,
            "重启到下载模式",
        );
        register(
            &mut commands,
            "REBOOT_SIDELOAD",
            &["reboot", "sideload"],
            false,
            RiskLevel::Dangerous,
            "重启到 Sideload 模式",
        );
        register(
            &mut commands,
            "REBOOT_SIDELOAD_AUTO",
            &["reboot", "sideload-auto-reboot"],
            false,
            RiskLevel::Dangerous,
            "重启到 Sideload (自动重启)",
        );

        // === 禁止命令 ===
        // fastboot flashing unlock 等高风险命令默认禁止
        // 可在后续版本中支持解锁

        Self { commands }
    }

    /// 检查命令是否允许执行，返回实际的 ADB 参数
    pub fn check_and_build_args(
        &self,
        command_name: &str,
        user_args: &[String],
    ) -> Result<(Vec<String>, RiskLevel), AppError> {
        let def = self.commands.get(command_name).ok_or_else(|| {
            warn!("Unknown command: {}", command_name);
            AppError::CommandBlocked(format!(
                "Unknown command '{}'. Use a registered command name.",
                command_name
            ))
        })?;

        if def.risk_level == RiskLevel::Blocked {
            return Err(AppError::CommandBlocked(format!(
                "Command '{}' is blocked by safety policy",
                command_name
            )));
        }

        let mut args = def.adb_args.clone();

        if def.allow_extra_args {
            // 基本注入检查
            for arg in user_args {
                if arg.contains("&&")
                    || arg.contains("||")
                    || arg.contains(';')
                    || arg.contains('|')
                {
                    return Err(AppError::CommandBlocked(format!(
                        "Shell injection detected in argument: '{}'",
                        arg
                    )));
                }
            }
            args.extend(user_args.iter().cloned());
        }

        Ok((args, def.risk_level.clone()))
    }

    /// 获取所有已注册的命令列表
    pub fn list_commands(&self) -> Vec<CommandInfo> {
        self.commands
            .iter()
            .map(|(name, def)| CommandInfo {
                name: name.clone(),
                risk_level: def.risk_level.clone(),
                allow_extra_args: def.allow_extra_args,
                description: def.description.clone(),
            })
            .collect()
    }
}

impl Default for SafetyPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// 命令信息（前端可展示）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub risk_level: RiskLevel,
    pub allow_extra_args: bool,
    pub description: String,
}

fn register(
    map: &mut HashMap<String, CommandDef>,
    name: &str,
    adb_args: &[&str],
    allow_extra_args: bool,
    risk_level: RiskLevel,
    description: &str,
) {
    map.insert(
        name.to_string(),
        CommandDef {
            adb_args: adb_args.iter().map(|s| s.to_string()).collect(),
            allow_extra_args,
            risk_level,
            description: description.to_string(),
        },
    );
}

/// 命令执行策略 — 细粒度控制单个命令的安全约束
#[derive(Debug, Clone)]
pub struct CommandPolicy {
    /// 是否允许管道操作（|）
    pub allow_pipe: bool,
    /// 是否允许重定向（>）
    pub allow_redirect: bool,
    /// 是否允许命令替换（$(cmd) 或 `cmd`）
    pub allow_subshell: bool,
    /// 最大参数数量（0 = 不限制）
    pub max_args: usize,
    /// 是否需要设备连接
    pub require_device: bool,
    /// 是否需要 root 权限
    pub require_root: bool,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        Self {
            allow_pipe: false,
            allow_redirect: false,
            allow_subshell: false,
            max_args: 0,
            require_device: true,
            require_root: false,
        }
    }
}

/// 安全策略的推荐预设
impl CommandPolicy {
    /// 最宽松：仅检查危险字符（shell 命令用）
    pub fn permissive() -> Self {
        Self {
            allow_pipe: true,
            allow_redirect: true,
            allow_subshell: false,
            max_args: 50,
            require_device: true,
            require_root: false,
        }
    }

    /// 严格：不允许任何 shell 特性
    pub fn strict() -> Self {
        Self::default()
    }

    /// 需要 root 权限的操作
    pub fn requires_root() -> Self {
        Self {
            require_root: true,
            ..Self::default()
        }
    }
}

/// 验证用户提供的额外参数是否安全
///
/// 返回 Err 如果检测到危险模式，Ok 如果通过检查。
pub fn validate_command(cmd: &str, policy: &CommandPolicy) -> Result<(), AppError> {
    // 1. 检查危险字符
    if !policy.allow_pipe && (cmd.contains('|') || cmd.contains("|&")) {
        return Err(AppError::CommandBlocked("不允许管道操作 (|)".into()));
    }
    if !policy.allow_redirect && (cmd.contains('>') || cmd.contains('<')) {
        return Err(AppError::CommandBlocked("不允许重定向操作 (>/<)".into()));
    }
    if !policy.allow_subshell && (cmd.contains("$(") || cmd.contains('`')) {
        return Err(AppError::CommandBlocked("不允许命令替换 ($() / ``)".into()));
    }

    // 2. 检查常见注入模式
    let dangerous_patterns = ["&&", "||", ";", "\n", "%0a", "%0d%0a"];
    for pattern in &dangerous_patterns {
        if cmd.to_lowercase().contains(pattern) {
            return Err(AppError::CommandBlocked(format!(
                "检测到命令注入模式: '{}'",
                pattern
            )));
        }
    }

    Ok(())
}

/// 验证参数数量
pub fn validate_arg_count(args: &[String], policy: &CommandPolicy) -> Result<(), AppError> {
    if policy.max_args > 0 && args.len() > policy.max_args {
        return Err(AppError::CommandBlocked(format!(
            "参数过多 (最多 {} 个, 当前 {})",
            policy.max_args,
            args.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command() {
        let policy = SafetyPolicy::new();
        let (args, level) = policy.check_and_build_args("LIST_DEVICES", &[]).unwrap();
        assert_eq!(args, vec!["devices", "-l"]);
        assert_eq!(level, RiskLevel::Safe);
    }

    #[test]
    fn test_command_with_extra_args() {
        let policy = SafetyPolicy::new();
        let (args, level) = policy
            .check_and_build_args(
                "SHELL",
                &["getprop".to_string(), "ro.build.display.id".to_string()],
            )
            .unwrap();
        assert_eq!(args, vec!["shell", "getprop", "ro.build.display.id"]);
        assert_eq!(level, RiskLevel::Normal);
    }

    #[test]
    fn test_dangerous_command() {
        let policy = SafetyPolicy::new();
        let (args, level) = policy.check_and_build_args("REBOOT_RECOVERY", &[]).unwrap();
        assert_eq!(args, vec!["reboot", "recovery"]);
        assert_eq!(level, RiskLevel::Dangerous);
    }

    #[test]
    fn test_unknown_command_blocked() {
        let policy = SafetyPolicy::new();
        let result = policy.check_and_build_args("FORMAT_DATA", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_injection_blocked() {
        let policy = SafetyPolicy::new();
        let result = policy.check_and_build_args("SHELL", &["ls && rm -rf /".to_string()]);
        assert!(result.is_err());
    }
}
