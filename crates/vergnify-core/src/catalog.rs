// @author Cursor

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKey {
    /// Node.js や Java のように major が系列になる。
    Major,
    /// Python 3.13 や Go 1.27 のように major.minor が系列になる。
    MajorMinor,
    /// 保守系列が一つだけのツール。
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatestSource {
    NodeLines,
    GoLines,
    RustChannel,
    EndOfLife {
        product: &'static str,
        key: LineKey,
    },
    GitHub {
        owner: &'static str,
        repo: &'static str,
    },
    /// Flutter の安定版。GitHub の最新リリースは beta のタグになる。
    FlutterStable,
}

#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub binaries: &'static [&'static str],
    pub args: &'static [&'static str],
    pub line_key: LineKey,
    pub latest: LatestSource,
}

pub fn tools() -> &'static [ToolSpec] {
    &TOOLS
}

pub fn find(id: &str) -> Option<&'static ToolSpec> {
    TOOLS.iter().find(|tool| tool.id == id)
}

const TOOLS: [ToolSpec; 23] = [
    ToolSpec {
        id: "python",
        name: "Python",
        binaries: &["python3", "python"],
        args: &["--version"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::EndOfLife {
            product: "python",
            key: LineKey::MajorMinor,
        },
    },
    ToolSpec {
        id: "node",
        name: "Node.js",
        binaries: &["node"],
        args: &["--version"],
        line_key: LineKey::Major,
        latest: LatestSource::NodeLines,
    },
    ToolSpec {
        id: "deno",
        name: "Deno",
        binaries: &["deno"],
        args: &["--version"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::EndOfLife {
            product: "deno",
            key: LineKey::MajorMinor,
        },
    },
    ToolSpec {
        id: "bun",
        name: "Bun",
        binaries: &["bun"],
        args: &["--version"],
        line_key: LineKey::Major,
        latest: LatestSource::EndOfLife {
            product: "bun",
            key: LineKey::Major,
        },
    },
    ToolSpec {
        id: "php",
        name: "PHP",
        binaries: &["php"],
        args: &["--version"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::EndOfLife {
            product: "php",
            key: LineKey::MajorMinor,
        },
    },
    ToolSpec {
        id: "go",
        name: "Go",
        binaries: &["go"],
        args: &["version"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::GoLines,
    },
    ToolSpec {
        id: "rust",
        name: "Rust",
        binaries: &["rustc"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::RustChannel,
    },
    ToolSpec {
        id: "ruby",
        name: "Ruby",
        binaries: &["ruby"],
        args: &["--version"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::EndOfLife {
            product: "ruby",
            key: LineKey::MajorMinor,
        },
    },
    ToolSpec {
        id: "elixir",
        name: "Elixir",
        binaries: &["elixir"],
        args: &["--version"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::EndOfLife {
            product: "elixir",
            key: LineKey::MajorMinor,
        },
    },
    ToolSpec {
        id: "java",
        name: "Java",
        binaries: &["java"],
        args: &["-version"],
        line_key: LineKey::Major,
        latest: LatestSource::EndOfLife {
            product: "temurin",
            key: LineKey::Major,
        },
    },
    ToolSpec {
        id: "dotnet",
        name: ".NET",
        binaries: &["dotnet"],
        args: &["--version"],
        line_key: LineKey::Major,
        latest: LatestSource::EndOfLife {
            product: "dotnet",
            key: LineKey::Major,
        },
    },
    ToolSpec {
        id: "swift",
        name: "Swift",
        binaries: &["swift"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "swiftlang",
            repo: "swift",
        },
    },
    ToolSpec {
        id: "flutter",
        name: "Flutter",
        binaries: &["flutter"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::FlutterStable,
    },
    ToolSpec {
        id: "docker",
        name: "Docker",
        binaries: &["docker"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "moby",
            repo: "moby",
        },
    },
    ToolSpec {
        id: "podman",
        name: "Podman",
        binaries: &["podman"],
        args: &["--version"],
        line_key: LineKey::Major,
        latest: LatestSource::GitHub {
            owner: "containers",
            repo: "podman",
        },
    },
    ToolSpec {
        id: "git",
        name: "Git",
        binaries: &["git"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "git",
            repo: "git",
        },
    },
    ToolSpec {
        id: "gh",
        name: "GitHub CLI",
        binaries: &["gh"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "cli",
            repo: "cli",
        },
    },
    ToolSpec {
        id: "terraform",
        name: "Terraform",
        binaries: &["terraform"],
        args: &["version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "hashicorp",
            repo: "terraform",
        },
    },
    ToolSpec {
        id: "kubectl",
        name: "Kubectl",
        binaries: &["kubectl"],
        args: &["version", "--client"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::EndOfLife {
            product: "kubernetes",
            key: LineKey::MajorMinor,
        },
    },
    ToolSpec {
        id: "helm",
        name: "Helm",
        binaries: &["helm"],
        args: &["version", "--short"],
        line_key: LineKey::MajorMinor,
        latest: LatestSource::GitHub {
            owner: "helm",
            repo: "helm",
        },
    },
    ToolSpec {
        id: "aws",
        name: "AWS CLI",
        binaries: &["aws"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "aws",
            repo: "aws-cli",
        },
    },
    ToolSpec {
        id: "az",
        name: "Azure CLI",
        binaries: &["az"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "Azure",
            repo: "azure-cli",
        },
    },
    ToolSpec {
        id: "uv",
        name: "uv",
        binaries: &["uv"],
        args: &["--version"],
        line_key: LineKey::Single,
        latest: LatestSource::GitHub {
            owner: "astral-sh",
            repo: "uv",
        },
    },
];
