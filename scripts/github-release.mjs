import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const package_path = resolve(root, "package.json");
const cargo_path = resolve(root, "src-tauri", "Cargo.toml");
const tauri_config_path = resolve(root, "src-tauri", "tauri.conf.json");
const bundle_root = resolve(root, "src-tauri", "target", "release", "bundle");
const release_notes_root = resolve(root, "release-notes");
const dry_run = process.argv.includes("--dry-run");

function read_json(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function output_text(value) {
  if (typeof value === "string") return value;
  if (Buffer.isBuffer(value)) return value.toString("utf8");
  return "";
}

function command_error_message(command, args, result) {
  const output = [output_text(result.stdout), output_text(result.stderr)]
    .filter(Boolean)
    .join("\n")
    .trim();
  const error = result.error?.message ? `\n${result.error.message}` : "";
  return `${command} ${args.join(" ")} 执行失败${output ? `\n${output}` : ""}${error}`;
}

function capture(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    shell: false,
  });

  if (result.error || result.status !== 0) {
    throw new Error(command_error_message(command, args, result));
  }

  return output_text(result.stdout).trim();
}

function run(command, args) {
  if (dry_run) {
    console.log(`[dry-run] ${command} ${args.join(" ")}`);
    return;
  }

  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    shell: false,
  });

  if (result.error || result.status !== 0) {
    throw new Error(command_error_message(command, args, result));
  }
}

function try_capture(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    shell: false,
  });

  return {
    ok: !result.error && result.status === 0,
    output: output_text(result.stdout).trim(),
    error: [output_text(result.stderr), result.error?.message ?? ""].filter(Boolean).join("\n").trim(),
  };
}

function ensure_clean_worktree() {
  const status = capture("git", ["status", "--porcelain"]);
  if (status) {
    throw new Error(`工作区存在未提交修改，请先提交后再发布。\n${status}`);
  }
}

function read_cargo_version() {
  const content = readFileSync(cargo_path, "utf8");
  const match = content.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error("无法读取 src-tauri/Cargo.toml 版本号");
  }
  return match[1];
}

function read_versions() {
  const package_version = read_json(package_path).version;
  const cargo_version = read_cargo_version();
  const tauri_version = read_json(tauri_config_path).version;

  if (package_version !== cargo_version || package_version !== tauri_version) {
    throw new Error(
      [
        "版本号不一致，停止发布。",
        `package.json: ${package_version}`,
        `src-tauri/Cargo.toml: ${cargo_version}`,
        `src-tauri/tauri.conf.json: ${tauri_version}`,
      ].join("\n"),
    );
  }

  return package_version;
}

function artifact_name_matches_version(file, version) {
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`(^|[_\\-])${escaped}([_\\-.]|$)`).test(file);
}

function find_artifacts(kind, extension, version) {
  const dir = resolve(bundle_root, kind);
  if (!existsSync(dir)) {
    return [];
  }

  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => resolve(dir, entry.name))
    .filter((file) => extname(file).toLowerCase() === extension)
    .filter((file) => artifact_name_matches_version(file, version));
}

function ensure_artifacts(version) {
  const msi = find_artifacts("msi", ".msi", version);
  const nsis = find_artifacts("nsis", ".exe", version);

  if (!msi.length || !nsis.length) {
    const expected_msi = resolve(bundle_root, "msi", `*${version}*.msi`);
    const expected_nsis = resolve(bundle_root, "nsis", `*${version}*.exe`);
    throw new Error(
      [
        `没有找到当前版本 ${version} 的完整构建产物，停止发布。`,
        `MSI: ${msi.length ? msi.join(", ") : `未找到，期望 ${expected_msi}`}`,
        `NSIS: ${nsis.length ? nsis.join(", ") : `未找到，期望 ${expected_nsis}`}`,
        "请先执行 pnpm tauri:build 或 pnpm release:build。",
      ].join("\n"),
    );
  }

  return [...msi, ...nsis];
}

function find_release_notes(tag, version) {
  const candidates = [
    resolve(release_notes_root, `${tag}.md`),
    resolve(release_notes_root, `${version}.md`),
  ];

  return candidates.find((path) => existsSync(path));
}

function ensure_gh_available() {
  const version = try_capture("gh", ["--version"]);
  if (!version.ok) {
    throw new Error("未找到 GitHub CLI，请先安装 gh 并重新打开终端。");
  }

  const auth = try_capture("gh", ["auth", "status"]);
  if (!auth.ok) {
    throw new Error(`gh 尚未登录或无权限，请先执行 gh auth login。\n${auth.error || auth.output}`);
  }
}

function ensure_tag(tag) {
  const head = capture("git", ["rev-parse", "HEAD"]);
  const remote_tag = try_capture("git", ["ls-remote", "--tags", "origin", `refs/tags/${tag}`]);
  let local_tag = try_capture("git", ["rev-list", "-n", "1", tag]);

  if (remote_tag.output && !local_tag.ok) {
    run("git", ["fetch", "origin", `refs/tags/${tag}:refs/tags/${tag}`]);
    local_tag = try_capture("git", ["rev-list", "-n", "1", tag]);
  }

  if (local_tag.ok && local_tag.output !== head) {
    throw new Error(`本地标签 ${tag} 不指向当前提交，停止发布。`);
  }

  if (!local_tag.ok) {
    run("git", ["tag", tag]);
  }

  if (!remote_tag.output) {
    run("git", ["push", "origin", tag]);
  }
}

function publish_release(tag, version, artifacts, release_notes_path) {
  const release = try_capture("gh", ["release", "view", tag]);
  if (release.ok) {
    if (release_notes_path) {
      run("gh", ["release", "edit", tag, "--notes-file", release_notes_path]);
    }
    run("gh", ["release", "upload", tag, ...artifacts, "--clobber"]);
    return;
  }

  const args = ["release", "create", tag, ...artifacts, "--title", tag];
  if (release_notes_path) {
    args.push("--notes-file", release_notes_path);
  }

  run("gh", args);
}

try {
  ensure_clean_worktree();

  const version = read_versions();
  const tag = `v${version}`;
  const artifacts = ensure_artifacts(version);
  const release_notes_path = find_release_notes(tag, version);

  ensure_gh_available();

  console.log(`准备发布 ${tag}`);
  if (release_notes_path) {
    console.log(`将使用更新说明: ${release_notes_path}`);
  } else {
    console.log(`未找到 ${tag}.md 或 ${version}.md，发布时不携带更新说明。`);
  }
  console.log("将上传以下构建产物:");
  for (const artifact of artifacts) {
    console.log(`- ${artifact}`);
  }

  ensure_tag(tag);
  publish_release(tag, version, artifacts, release_notes_path);

  console.log(`GitHub Release ${tag} 发布完成。`);
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
