import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const package_path = resolve(root, "package.json");
const cargo_path = resolve(root, "src-tauri", "Cargo.toml");
const tauri_config_path = resolve(root, "src-tauri", "tauri.conf.json");

const bump = process.argv[2] ?? "patch";

function write_json(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function parse_version(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/.exec(version);
  if (!match) {
    throw new Error(`版本号格式无效: ${version}`);
  }

  return match.slice(1).map(Number);
}

function next_version(current, mode) {
  if (/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(mode)) {
    return mode;
  }

  const [major, minor, patch] = parse_version(current);
  switch (mode) {
    case "major":
      return `${major + 1}.0.0`;
    case "minor":
      return `${major}.${minor + 1}.0`;
    case "patch":
      return `${major}.${minor}.${patch + 1}`;
    default:
      throw new Error(`不支持的版本更新类型: ${mode}，请使用 patch、minor、major 或明确版本号`);
  }
}

function replace_required(content, pattern, replacement, label) {
  if (!pattern.test(content)) {
    throw new Error(`无法在 ${label} 中找到版本号`);
  }
  return content.replace(pattern, replacement);
}

function run(command, args, cwd) {
  const executable = process.platform === "win32" ? `${command}.cmd` : command;
  const result = spawnSync(executable, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 64,
    stdio: ["inherit", "pipe", "pipe"],
  });

  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }

  if (result.error) {
    throw new Error(
      [
        `${command} ${args.join(" ")} 启动失败`,
        `工作目录: ${cwd}`,
        `错误信息: ${result.error.message}`,
      ].join("\n"),
    );
  }

  if (result.status !== 0) {
    throw new Error(
      [
        `${command} ${args.join(" ")} 执行失败`,
        `工作目录: ${cwd}`,
        `退出码: ${result.status ?? "无"}`,
        `信号: ${result.signal ?? "无"}`,
      ].join("\n"),
    );
  }
}

function restore_files(files) {
  for (const file of files) {
    writeFileSync(file.path, file.content);
  }
}

const original_files = [
  { path: package_path, content: readFileSync(package_path, "utf8") },
  { path: cargo_path, content: readFileSync(cargo_path, "utf8") },
  { path: tauri_config_path, content: readFileSync(tauri_config_path, "utf8") },
];

try {
  const package_json = JSON.parse(original_files[0].content);
  const version = next_version(package_json.version, bump);

  package_json.version = version;
  write_json(package_path, package_json);

  writeFileSync(
    cargo_path,
    replace_required(
      original_files[1].content,
      /^(version\s*=\s*)"[^"]+"/m,
      `$1"${version}"`,
      "Cargo.toml",
    ),
  );

  const tauri_config = JSON.parse(original_files[2].content);
  tauri_config.version = version;
  write_json(tauri_config_path, tauri_config);

  console.log(`版本号已更新为 ${version}`);
  run("pnpm", ["tauri:build"], root);
} catch (error) {
  try {
    restore_files(original_files);
    console.error("发布失败，版本号已回退到执行前状态。");
  } catch (restore_error) {
    console.error("发布失败，并且版本号回退失败，请手动检查版本文件。");
    console.error(restore_error);
  }

  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
