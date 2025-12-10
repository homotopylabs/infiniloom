//! Security scanning for secrets and sensitive data

use regex::Regex;
use std::collections::HashSet;

/// A detected secret or sensitive data
#[derive(Debug, Clone)]
pub struct SecretFinding {
    /// Type of secret
    pub kind: SecretKind,
    /// File path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Matched pattern (redacted)
    pub pattern: String,
    /// Severity level
    pub severity: Severity,
}

/// Kind of secret detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// API key
    ApiKey,
    /// Access token
    AccessToken,
    /// Private key
    PrivateKey,
    /// Password
    Password,
    /// Database connection string
    ConnectionString,
    /// AWS credentials
    AwsCredential,
    /// GitHub token
    GitHubToken,
    /// Generic secret
    Generic,
}

impl SecretKind {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::ApiKey => "API Key",
            Self::AccessToken => "Access Token",
            Self::PrivateKey => "Private Key",
            Self::Password => "Password",
            Self::ConnectionString => "Connection String",
            Self::AwsCredential => "AWS Credential",
            Self::GitHubToken => "GitHub Token",
            Self::Generic => "Generic Secret",
        }
    }
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Security scanner
pub struct SecurityScanner {
    patterns: Vec<SecretPattern>,
    allowlist: HashSet<String>,
}

struct SecretPattern {
    kind: SecretKind,
    regex: Regex,
    severity: Severity,
}

impl Default for SecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityScanner {
    /// Create a new security scanner with default patterns
    pub fn new() -> Self {
        let patterns = vec![
            // AWS
            SecretPattern {
                kind: SecretKind::AwsCredential,
                regex: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
                severity: Severity::Critical,
            },
            SecretPattern {
                kind: SecretKind::AwsCredential,
                regex: Regex::new(r#"(?i)aws[_-]?secret[_-]?access[_-]?key['"]?\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})"#).unwrap(),
                severity: Severity::Critical,
            },
            // GitHub
            SecretPattern {
                kind: SecretKind::GitHubToken,
                regex: Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(),
                severity: Severity::Critical,
            },
            SecretPattern {
                kind: SecretKind::GitHubToken,
                regex: Regex::new(r"github_pat_[A-Za-z0-9]{22}_[A-Za-z0-9]{59}").unwrap(),
                severity: Severity::Critical,
            },
            // Private keys
            SecretPattern {
                kind: SecretKind::PrivateKey,
                regex: Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----").unwrap(),
                severity: Severity::Critical,
            },
            // Generic API keys
            SecretPattern {
                kind: SecretKind::ApiKey,
                regex: Regex::new(r#"(?i)(?:api[_-]?key|apikey)['"]?\s*[:=]\s*['"]?([A-Za-z0-9_-]{20,})"#).unwrap(),
                severity: Severity::High,
            },
            // Generic secrets
            SecretPattern {
                kind: SecretKind::Generic,
                regex: Regex::new(r#"(?i)(?:secret|token)['"]?\s*[:=]\s*['"]?([A-Za-z0-9_-]{20,})"#).unwrap(),
                severity: Severity::High,
            },
            // Passwords
            SecretPattern {
                kind: SecretKind::Password,
                regex: Regex::new(r#"(?i)password['"]?\s*[:=]\s*['"]?([^'"\s]{8,})"#).unwrap(),
                severity: Severity::High,
            },
            // Connection strings
            SecretPattern {
                kind: SecretKind::ConnectionString,
                regex: Regex::new(r#"(?i)(?:mongodb|postgres|mysql|redis)://[^\s'""]+"#).unwrap(),
                severity: Severity::High,
            },
            // JWT tokens
            SecretPattern {
                kind: SecretKind::AccessToken,
                regex: Regex::new(r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*").unwrap(),
                severity: Severity::High,
            },
            // Slack tokens
            SecretPattern {
                kind: SecretKind::AccessToken,
                regex: Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}-[a-zA-Z0-9]{24}").unwrap(),
                severity: Severity::High,
            },
            // Stripe keys
            SecretPattern {
                kind: SecretKind::ApiKey,
                regex: Regex::new(r"(?:sk|pk)_(?:test|live)_[A-Za-z0-9]{24,}").unwrap(),
                severity: Severity::Critical,
            },
        ];

        Self {
            patterns,
            allowlist: HashSet::new(),
        }
    }

    /// Add a pattern to allowlist
    pub fn allowlist(&mut self, pattern: &str) {
        self.allowlist.insert(pattern.to_owned());
    }

    /// Scan content for secrets
    pub fn scan(&self, content: &str, file_path: &str) -> Vec<SecretFinding> {
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Skip comments and common false positives
            let trimmed = line.trim();
            if trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with("*")
                || trimmed.contains("example")
                || trimmed.contains("placeholder")
                || trimmed.contains("xxxxx")
            {
                continue;
            }

            for pattern in &self.patterns {
                if let Some(m) = pattern.regex.find(line) {
                    let matched = m.as_str();

                    // Check allowlist
                    if self.allowlist.iter().any(|a| matched.contains(a)) {
                        continue;
                    }

                    findings.push(SecretFinding {
                        kind: pattern.kind,
                        file: file_path.to_owned(),
                        line: (line_num + 1) as u32,
                        pattern: redact(matched),
                        severity: pattern.severity,
                    });
                }
            }
        }

        findings
    }

    /// Scan a file and return whether it's safe to include
    pub fn is_safe(&self, content: &str, file_path: &str) -> bool {
        let findings = self.scan(content, file_path);
        findings.iter().all(|f| f.severity < Severity::High)
    }

    /// Get summary of findings
    pub fn summarize(findings: &[SecretFinding]) -> String {
        if findings.is_empty() {
            return "No secrets detected".to_owned();
        }

        let critical = findings
            .iter()
            .filter(|f| f.severity == Severity::Critical)
            .count();
        let high = findings
            .iter()
            .filter(|f| f.severity == Severity::High)
            .count();

        format!(
            "Found {} potential secrets ({} critical, {} high severity)",
            findings.len(),
            critical,
            high
        )
    }
}

/// Redact a matched secret for display
fn redact(s: &str) -> String {
    if s.len() <= 8 {
        return "*".repeat(s.len());
    }

    let prefix_len = 4.min(s.len() / 4);
    let suffix_len = 4.min(s.len() / 4);

    format!(
        "{}{}{}",
        &s[..prefix_len],
        "*".repeat(s.len() - prefix_len - suffix_len),
        &s[s.len() - suffix_len..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_key_detection() {
        let scanner = SecurityScanner::new();
        let content = r#"AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE""#;

        let findings = scanner.scan(content, "config.py");

        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.kind == SecretKind::AwsCredential));
    }

    #[test]
    fn test_github_token_detection() {
        let scanner = SecurityScanner::new();
        let content = r#"GITHUB_TOKEN = "ghp_abcdefghijklmnopqrstuvwxyz1234567890""#;

        let findings = scanner.scan(content, ".env");

        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.kind == SecretKind::GitHubToken));
    }

    #[test]
    fn test_private_key_detection() {
        let scanner = SecurityScanner::new();
        let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpA...";

        let findings = scanner.scan(content, "key.pem");

        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.kind == SecretKind::PrivateKey));
    }

    #[test]
    fn test_allowlist() {
        let mut scanner = SecurityScanner::new();
        scanner.allowlist("EXAMPLE");

        let content = r#"api_key = "AKIAIOSFODNN7EXAMPLE""#;
        let findings = scanner.scan(content, "test.py");

        assert!(findings.is_empty());
    }

    #[test]
    fn test_redact() {
        assert_eq!(redact("AKIAIOSFODNN7EXAMPLE"), "AKIA************MPLE");
        assert_eq!(redact("short"), "*****");
    }

    #[test]
    fn test_skip_comments() {
        let scanner = SecurityScanner::new();
        let content = "# api_key = 'some_secret_key_12345678901234567890'";

        let findings = scanner.scan(content, "test.py");

        assert!(findings.is_empty());
    }
}
