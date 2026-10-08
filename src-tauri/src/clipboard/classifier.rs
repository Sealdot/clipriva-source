pub const MAX_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_URL_BYTES: usize = 8 * 1024;
pub const MAX_COLOR_BYTES: usize = 1024;

/// Classifies the small, local-only text family supported by the v1.5
/// clipboard model. The order is intentional: a colour or URL must not be
/// mistaken for a generic code fragment just because it contains punctuation.
pub fn classify_text(content: &str) -> &'static str {
    let value = content.trim();

    if is_color(value) {
        return "color";
    }
    if is_http_url(value) {
        return "url";
    }
    if looks_like_code(value) {
        return "code";
    }
    if looks_like_command(value) {
        return "command";
    }
    "text"
}

/// Applies the normalization promised by the content model before a text clip
/// is persisted. Newlines stay meaningful, while repeated horizontal
/// whitespace no longer produces visually identical history records.
///
/// Code and commands are deliberately left byte-for-byte intact: indentation,
/// arguments, and spacing can carry meaning there. URLs and colours are also
/// preserved exactly so a Copy action round-trips the value the user selected.
pub fn normalize_text_for_storage(content: &str, kind: &str) -> String {
    if kind != "text" {
        return content.to_owned();
    }

    let normalized_lines = content.replace("\r\n", "\n").replace('\r', "\n");
    let mut normalized = String::with_capacity(normalized_lines.len());
    let mut previous_was_horizontal_whitespace = false;
    for character in normalized_lines.chars() {
        if character == '\n' {
            while normalized.ends_with(' ') {
                normalized.pop();
            }
            normalized.push(character);
            previous_was_horizontal_whitespace = false;
        } else if character.is_whitespace() {
            if !previous_was_horizontal_whitespace {
                normalized.push(' ');
                previous_was_horizontal_whitespace = true;
            }
        } else {
            normalized.push(character);
            previous_was_horizontal_whitespace = false;
        }
    }

    // Do not use `trim()` here: leading and trailing newlines are a meaningful
    // part of a copied text block. Only the normalization-inserted horizontal
    // spaces at the outer boundary can be discarded.
    normalized
        .trim_start_matches(' ')
        .trim_end_matches(' ')
        .to_owned()
}

pub fn max_bytes_for_kind(kind: &str) -> usize {
    match kind {
        "url" => MAX_URL_BYTES,
        "color" => MAX_COLOR_BYTES,
        // Rich text and images use their own representation-specific limits.
        "text" | "code" | "command" => MAX_TEXT_BYTES,
        _ => MAX_TEXT_BYTES,
    }
}

fn is_color(value: &str) -> bool {
    is_hex_color(value) || is_function_color(value, "rgb") || is_function_color(value, "hsl")
}

fn is_hex_color(value: &str) -> bool {
    matches!(value.len(), 4 | 7 | 9)
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn is_function_color(value: &str, function: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let Some(arguments) = lower
        .strip_prefix(function)
        .and_then(|remainder| remainder.strip_prefix('('))
        .and_then(|remainder| remainder.strip_suffix(')'))
    else {
        return false;
    };

    let values = arguments.split(',').map(str::trim).collect::<Vec<_>>();
    if !(3..=4).contains(&values.len()) || values.iter().any(|value| value.is_empty()) {
        return false;
    }

    values.iter().all(|value| {
        value.chars().all(|character| {
            character.is_ascii_digit() || matches!(character, '.' | '%' | '+' | '-')
        })
    })
}

fn is_http_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let Some(remainder) = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
    else {
        return false;
    };

    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty()
        && !authority.starts_with('@')
        && !authority.chars().any(char::is_whitespace)
}

fn looks_like_code(value: &str) -> bool {
    const CODE_PREFIXES: [&str; 17] = [
        "const ",
        "let ",
        "var ",
        "fn ",
        "def ",
        "class ",
        "function ",
        "import ",
        "export ",
        "from ",
        "use ",
        "public ",
        "private ",
        "select ",
        "insert ",
        "update ",
        "#include",
    ];
    let lower = value.to_ascii_lowercase();

    lower.lines().any(|line| {
        let line = line.trim_start();
        CODE_PREFIXES.iter().any(|prefix| line.starts_with(prefix))
    }) || (value.contains('\n')
        && (value.contains('{')
            || value.contains("=>")
            || value
                .lines()
                .any(|line| line.starts_with("    ") || line.starts_with('\t'))))
}

/// Recognizes only command lines with local, deterministic evidence. A known
/// executable alone is not enough unless it is a conventional no-argument
/// command; arguments must otherwise look like shell syntax or a compact CLI
/// invocation. This deliberately leaves ambiguous prose as plain text.
fn looks_like_command(value: &str) -> bool {
    if value.is_empty() || value.len() > 4 * 1024 || value.contains(['\n', '\r', '\0']) {
        return false;
    }

    let (candidate, has_prompt) = value
        .strip_prefix("$ ")
        .or_else(|| value.strip_prefix("% "))
        .map_or((value, false), |command| (command.trim_start(), true));
    let tokens = candidate.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        return false;
    }

    let mut executable_index = 0;
    let mut has_wrapper = false;
    if is_shell_wrapper(tokens[0]) {
        has_wrapper = true;
        executable_index += 1;
        while let Some(token) = tokens.get(executable_index) {
            if token.starts_with('-') || token.contains('=') {
                executable_index += 1;
            } else {
                break;
            }
        }
    }

    let Some(executable) = tokens.get(executable_index).copied() else {
        return false;
    };
    let explicit_path = is_explicit_executable_path(executable);
    if !explicit_path && !is_known_command(executable) {
        return false;
    }

    if has_prompt || has_wrapper || explicit_path {
        return true;
    }

    let arguments = &tokens[executable_index + 1..];
    if arguments.is_empty() {
        return matches!(executable, "clear" | "ls" | "pwd" | "whoami");
    }

    if ends_like_sentence(candidate) && !has_explicit_shell_syntax(candidate, arguments) {
        return false;
    }
    if has_explicit_shell_syntax(candidate, arguments) {
        return true;
    }

    if matches!(
        executable,
        "cat" | "cd" | "curl" | "echo" | "grep" | "mkdir" | "rm" | "rmdir" | "touch" | "wget"
    ) {
        return arguments.len() == 1;
    }
    if matches!(executable, "cp" | "mv" | "scp") {
        return arguments.len() == 2;
    }

    arguments.len() <= 2 && is_known_subcommand(arguments[0])
}

fn is_shell_wrapper(value: &str) -> bool {
    matches!(value, "command" | "env" | "nohup" | "sudo")
}

fn is_explicit_executable_path(value: &str) -> bool {
    value
        .strip_prefix("./")
        .or_else(|| value.strip_prefix("../"))
        .is_some_and(|remainder| !remainder.is_empty())
        || [
            "/bin/",
            "/opt/homebrew/bin/",
            "/usr/bin/",
            "/usr/local/bin/",
        ]
        .iter()
        .any(|prefix| value.starts_with(prefix) && value.len() > prefix.len())
}

fn is_known_command(value: &str) -> bool {
    matches!(
        value,
        "apt"
            | "apt-get"
            | "awk"
            | "brew"
            | "bun"
            | "cargo"
            | "cat"
            | "cd"
            | "chmod"
            | "chown"
            | "clear"
            | "cmake"
            | "cp"
            | "curl"
            | "deno"
            | "dnf"
            | "docker"
            | "echo"
            | "gh"
            | "git"
            | "go"
            | "gradle"
            | "grep"
            | "helm"
            | "java"
            | "javac"
            | "jq"
            | "just"
            | "kubectl"
            | "ls"
            | "make"
            | "mkdir"
            | "mvn"
            | "mv"
            | "node"
            | "npm"
            | "npx"
            | "pip"
            | "pip3"
            | "pnpm"
            | "podman"
            | "pwd"
            | "python"
            | "python3"
            | "rg"
            | "rm"
            | "rmdir"
            | "rsync"
            | "rustc"
            | "rustup"
            | "scp"
            | "sed"
            | "ssh"
            | "terraform"
            | "touch"
            | "uv"
            | "wget"
            | "whoami"
            | "xargs"
            | "yarn"
            | "yum"
    )
}

fn is_known_subcommand(value: &str) -> bool {
    matches!(
        value,
        "add"
            | "apply"
            | "audit"
            | "branch"
            | "build"
            | "check"
            | "checkout"
            | "clean"
            | "clone"
            | "clippy"
            | "commit"
            | "compose"
            | "create"
            | "delete"
            | "describe"
            | "dev"
            | "diff"
            | "down"
            | "exec"
            | "fetch"
            | "fmt"
            | "get"
            | "images"
            | "info"
            | "init"
            | "inspect"
            | "install"
            | "list"
            | "log"
            | "login"
            | "logout"
            | "logs"
            | "merge"
            | "new"
            | "prune"
            | "ps"
            | "publish"
            | "pull"
            | "push"
            | "rebase"
            | "remote"
            | "run"
            | "search"
            | "set"
            | "show"
            | "start"
            | "stash"
            | "status"
            | "switch"
            | "tag"
            | "test"
            | "uninstall"
            | "up"
            | "update"
            | "upgrade"
            | "version"
    )
}

fn has_explicit_shell_syntax(candidate: &str, arguments: &[&str]) -> bool {
    candidate.contains(" && ")
        || candidate.contains(" || ")
        || candidate.contains(" | ")
        || candidate.contains(" > ")
        || candidate.contains(" < ")
        || candidate.contains("$(")
        || candidate.contains('`')
        || arguments.iter().any(|argument| {
            argument.starts_with('-')
                || argument.starts_with('/')
                || argument.starts_with("./")
                || argument.starts_with("../")
                || argument.starts_with("~/")
                || argument.starts_with("http://")
                || argument.starts_with("https://")
                || argument.starts_with('\'')
                || argument.starts_with('"')
                || argument.contains('=')
        })
}

fn ends_like_sentence(value: &str) -> bool {
    value.ends_with(['.', '!', '?'])
}

#[cfg(test)]
mod tests {
    use super::{classify_text, max_bytes_for_kind, normalize_text_for_storage};

    #[test]
    fn recognizes_supported_text_kinds() {
        assert_eq!(classify_text("#6EE7B7"), "color");
        assert_eq!(classify_text("rgb(99, 230, 190)"), "color");
        assert_eq!(classify_text("hsl(160, 71%, 65%)"), "color");
        assert_eq!(classify_text("https://clipriva.com"), "url");
        assert_eq!(classify_text("HTTPS://clipriva.com/docs?q=1"), "url");
        assert_eq!(classify_text("export const name = 'ClipRiva';"), "code");
        assert_eq!(classify_text("select * from clips"), "code");
        assert_eq!(classify_text("git status"), "command");
        assert_eq!(classify_text("pnpm run build"), "command");
        assert_eq!(classify_text("cargo test --locked"), "command");
        assert_eq!(classify_text("$ ./scripts/release.sh --dry-run"), "command");
        assert_eq!(classify_text("ordinary clipboard text"), "text");
        assert_eq!(classify_text("https://"), "text");
    }

    #[test]
    fn leaves_ambiguous_command_like_prose_and_existing_kinds_unchanged() {
        assert_eq!(classify_text("Git status is useful in a standup."), "text");
        assert_eq!(classify_text("git status is clean today"), "text");
        assert_eq!(classify_text("open the project settings"), "text");
        assert_eq!(classify_text("please run pnpm test"), "text");
        assert_eq!(classify_text("https://clipriva.com/cli?command=git"), "url");
        assert_eq!(
            classify_text("export const command = 'git status';"),
            "code"
        );
        assert_eq!(
            classify_text("fn command() {\n    println!(\"run\");\n}"),
            "code"
        );
    }

    #[test]
    fn normalizes_plain_text_without_changing_code_or_meaningful_newlines() {
        assert_eq!(
            normalize_text_for_storage("  Copy\t this\r\nwith  spaces  ", "text"),
            "Copy this\nwith spaces"
        );
        assert_eq!(normalize_text_for_storage("\nCopy\n", "text"), "\nCopy\n");
        assert_eq!(
            normalize_text_for_storage("fn main() {\n\tprintln!(\"hi\");\n}", "code"),
            "fn main() {\n\tprintln!(\"hi\");\n}"
        );
        assert_eq!(max_bytes_for_kind("text"), 256 * 1024);
        assert_eq!(max_bytes_for_kind("command"), 256 * 1024);
        assert_eq!(max_bytes_for_kind("url"), 8 * 1024);
        assert_eq!(max_bytes_for_kind("color"), 1024);
    }
}
