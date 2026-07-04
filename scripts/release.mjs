import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const package_path = resolve(root, "package.json");
const cargo_path = resolve(root, "src-tauri", "Cargo.toml");
const tauri_config_path = resolve(root, "src-tauri", "tauri.conf.json");

const bump = process.argv[2] ?? "patch";

function read_json(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

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
  const result = spawnSync(command, args, {
    cwd,
    shell: process.platform === "win32",
    stdio: "inherit",
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} 执行失败`);
  }
}

const package_json = read_json(package_path);
const version = next_version(package_json.version, bump);

package_json.version = version;
write_json(package_path, package_json);

const cargo_toml = readFileSync(cargo_path, "utf8");
writeFileSync(
  cargo_path,
  replace_required(cargo_toml, /^(version\s*=\s*)"[^"]+"/m, `$1"${version}"`, "Cargo.toml"),
);

const tauri_config = read_json(tauri_config_path);
tauri_config.version = version;
write_json(tauri_config_path, tauri_config);

console.log(`版本号已更新为 ${version}`);
run("pnpm", ["tauri:build"], root);
